# jumpgate

jumpgate matches what each app needs to what a provider supplies, for one
target, and writes the result as a plan that holds references and never a
secret value.

An app declares what it runs, provides and needs in `mxv.toml`, and the
secrets it reads in `starmap.toml`. jumpgate turns those declarations into
need records, matches each need to a provide, and checks the plan, whoever
wrote it.

## Crates

| Crate | Holds |
| --- | --- |
| `crates/jumpgate` | The record types and their validation, the matcher and the plan. It runs no process and reads no secret store or environment variable, and its directory store is its only file access. |
| `crates/jumpgate-cli` | The `jumpgate` command, which also runs `board` to read and write the board store. |

## Records

| Type | What it holds | Path in a store |
| --- | --- | --- |
| `jumpgate/need/v1` | One thing an app requires. | `/jumpgate/needs/<app>/<name>` |
| `jumpgate/provide/v1` | Something a provider can supply, as a reference. | `/jumpgate/provides/<provider-kind>/<provider>/<name>` |
| `jumpgate/binding/v1` | A need matched to a provide for one target. | `/jumpgate/bindings/<target>/<app>/<need>` |
| `jumpgate/target/v1` | A target and the board whose bindings count for it. | `/jumpgate/targets/<name>` |
| `jumpgate/unit/v1` | A process or container that an app runs, with its start order and ports. | `/jumpgate/units/<app>/<unit>` |

Each record's JSON form starts with `"$type"`. The reader refuses an unknown
field, a missing field and a `$type` of another type, and parses every
string into its own type. A refusal carries a code of the form
`jumpgate::record::<reason>` and never quotes the refused text. The JSON
Schemas, draft 2020-12, are in `crates/jumpgate/schemas/jumpgate/`.

## What an app declares

In `mxv.toml`, `[needs.<name>]` says what the app requires: `repo` names the
repository of the app that provides it, `from` names what that app provides, `as` names the
variable that receives it, and `optional = true` lets the plan omit the need
when nothing binds it. The old spellings `[wants]` and `[offers]` are refused,
with a code that names the new spelling.

`[provides.<name>]` says what the app supplies. The first kind is `socket`,
with `unit`, which names a process or container of the app, and `path`, which is
relative to the checkout. `jumpgate needs` stores the absolute path under the
checkout it read, with every symlink in the checkout's own path resolved, so a
checkout reached through a link records the real directory. It refuses a path
longer than the platform's socket path limit (103 bytes on macOS, 107 on Linux),
with `jumpgate::declaration::socket_path`, and a checkout whose directory cannot
be resolved or is not UTF-8 text, with `jumpgate::declaration::provide`.
`jumpgate resolve` binds a need of that name to the stored path.

A need reaches the units of the app and, in the same way, the commands in its
`[tasks]`, so an app that has tasks and no process or container can declare a
need. The plan lists such bindings in a `commands` section by app and task name. A
secret reaches only a unit that declares it, and never a command.

## The store

Each command reads and writes records through a store. The default store is
the personal board. `--board <dir>` names another board, and its id for the
matcher is the last part of the directory's path, so `--board` with the
personal board's own directory gives that board a second id, and bindings
whose target authority is `personal` stop counting. Leave `--board` out to
use the personal board. `--store <dir>` names a directory of JSON files, one
for each record, which works with no board. `needs` creates that directory
when it writes and it is missing.

The board store runs the `board` command and touches no file of the board.
Reads are `board query` calls, and each write is one `board put --batch`
that registers the five record types at `/.board/types/jumpgate/<kind>/v1.schema`
from the crate's own schemas, writes the records and deletes the stale ones
together. A board that `board` cannot read, or that is not on `PATH`, fails
with `jumpgate::store::board`, and so does a query that `board` cut short at
its row limit, because the store never works from part of a board.

A stale need or provide that a binding in the store still names is kept, so the
board never holds a binding that points at nothing. The report of `needs`
lists it as kept, and the command fails with `jumpgate::need::kept` and exits
with 1, naming the binding to remove. Until the binding is removed, `resolve`
still reads the kept record like any other. The command sees the bindings of
the targets that have a target record. The board store refuses to read or
write a secret need that falls back to a literal, and the schema that it
registers for the need record makes `board` refuse such a need from any
writer.

## The command

`jumpgate needs --repo <path>... [--store <dir> | --board <dir>]` reads
`mxv.toml` and the `starmap.toml` files of each checkout and writes the
needs, provides and units they give to the store. A process reads the
`starmap.toml` that its optional `starmap` key names, as a path from the root
of the checkout, and otherwise the one in its `cwd` or at the root. A named
file that the checkout lacks is refused with
`jumpgate::declaration::starmap_missing`, and the command lists each
git-tracked `starmap.toml` that no process reads. Each `[service.<name>]` table
of `mxv.toml` that a process lists in `needs` gives a need for its endpoint
variable, such as `AWS_ENDPOINT_URL_DYNAMODB`, and the keys of the service's
`env`, and a provide `workspace/mxv/<name>` that the need requests. A service
that no process lists gives no record and is printed under `unused`. A
process that lists a service nobody declares is refused with
`jumpgate::declaration::service`, and two apps that declare one service name
with different kinds are both refused as a clash. A declaration it
refuses is reported with its code, file and line, and the command exits with
1 after writing everything else. The command replaces the records of each
app it reads without a refusal, so a need, provide or unit that the app no
longer declares is deleted. An app that the command does not read, and an app
whose declaration it refuses, keeps every record.

`jumpgate resolve --target <name> [--store <dir> | --board <dir>] [--out <file>]` matches the
needs in the store to its provides for the target record of that name and writes
the plan, which is `jumpgate/plan/v1alpha1`. The plan lists the needs that
nothing binds before the units, so what blocks it comes first. The command
writes the plan to `--out` through a temporary file and a rename, or prints it
without `--out`. It exits with 1, after writing the plan, when a need that
fails when unbound has no provide, when a binding names a provide that the store
lacks, when two provides meet one need, or when only a fallback value meets a
need on a cloud target, or when a service need has a cloud target and no
binding record, which the plan states as `local_only`. The matcher follows four rules in order: a binding
record from the target's authority board, a need, a starmap bind, then the
fallback of a local target. The JSON Schema of the plan is
`crates/jumpgate/schemas/jumpgate/plan/v1alpha1.schema.json`.

`jumpgate check <plan> [--store <dir> | --board <dir> | --plan-only]` reads
one plan file, whichever matcher wrote it, and runs six checks: the plan holds no secret value, every need is bound or says
what to do when unbound, every binding names a provide with a known provider, a
secret reaches only the units that declare it, a cloud plan has no
need met by a fallback, and every unit that a unit starts after exists with no
cycle in the start order. It lists the failed checks first, names the unit and
the need of each failure, never prints a value from the plan, and exits with 1
when a check failed or the file is not a plan. It writes nothing and reads no
secret store. Unless `--plan-only` is given, it compares the plan with the
store: the plan's target must exist with the plan's kind, each need the plan
binds or lists must exist with the kind and consumers the plan states, each
binding's provide must exist with the reference the plan states, and a need that
applies to the target, has a unit in the plan and is neither bound nor listed
is reported. Each of these is a reason of one of the six checks, with the code
`jumpgate::check::<reason>`. Each binding and each unbound entry in a plan states
the need's kind and the units that declare it, which `jumpgate resolve` copies
from the need record. Check 1 refuses any literal on a secret need, and any
literal that no fallback made. Check 4 refuses a secret delivered to a unit
that does not declare the need.

`jumpgate skills install` and `jumpgate completion install` install the agent
skill and the shell completion.

## Licence

Licensed under either of the Apache License, Version 2.0
([LICENSE-APACHE](LICENSE-APACHE)) or the MIT licence
([LICENSE-MIT](LICENSE-MIT)), at your option.
