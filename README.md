# jumpgate

jumpgate matches what each app needs to what a provider offers, for one
target, and writes the result as a plan that holds references and never a
secret value.

An app declares what it runs, provides and wants in `mxv.toml`, and the
secrets it reads in `starmap.toml`. jumpgate turns those declarations into
need records, matches each need to an offer, and checks the plan, whoever
wrote it.

## Crates

| Crate | Holds |
| --- | --- |
| `crates/jumpgate` | The record types and their validation, the matcher and the plan. It runs no process and reads no secret store or environment variable, and its directory store is its only file access. |
| `crates/jumpgate-cli` | The `jumpgate` command. |

## Records

| Type | What it holds | Path in a store |
| --- | --- | --- |
| `jumpgate/need/v1` | One thing an app requires. | `/jumpgate/needs/<app>/<name>` |
| `jumpgate/offer/v1` | Something a provider can supply, as a reference. | `/jumpgate/offers/<provider-kind>/<provider>/<name>` |
| `jumpgate/binding/v1` | A need matched to an offer for one target. | `/jumpgate/bindings/<target>/<app>/<need>` |
| `jumpgate/target/v1` | A target and the board whose bindings count for it. | `/jumpgate/targets/<name>` |
| `jumpgate/unit/v1` | A process or container that an app runs, with its start order and ports. | `/jumpgate/units/<app>/<unit>` |

Each record's JSON form starts with `"$type"`. The reader refuses an unknown
field, a missing field and a `$type` of another type, and parses every
string into its own type. A refusal carries a code of the form
`jumpgate::record::<reason>` and never quotes the refused text. The JSON
Schemas, draft 2020-12, are in `crates/jumpgate/schemas/jumpgate/`.

## The command

`jumpgate needs --repo <path>... --store <dir>` reads `mxv.toml` and the
`starmap.toml` files of each checkout and writes the needs, offers and units
they give, one JSON file for each, to the store directory. A declaration it
refuses is reported with its code, file and line, and the command exits with
1 after writing everything else. The command replaces the records of each
app it reads without a refusal, so a need, offer or unit that the app no
longer declares is deleted. An app that the command does not read, and an app
whose declaration it refuses, keeps every record.

`jumpgate resolve --target <name> --store <dir> [--out <file>]` matches the
needs in the store to its offers for the target record of that name and writes
the plan, which is `jumpgate/plan/v1alpha1`. The plan lists the needs that
nothing binds before the units, so what blocks it comes first. The command
writes the plan to `--out` through a temporary file and a rename, or prints it
without `--out`. It exits with 1, after writing the plan, when a need that
fails when unbound has no offer, when a binding names an offer that the store
lacks, when two offers meet one need, or when only a fallback value meets a
need on a cloud target. The matcher follows four rules in order: a binding
record from the target's authority board, a want, a starmap bind, then the
fallback of a local target. The JSON Schema of the plan is
`crates/jumpgate/schemas/jumpgate/plan/v1alpha1.schema.json`.

`jumpgate check <plan>` reads one plan file, whichever matcher wrote it, and
runs six checks: the plan holds no secret value, every need is bound or says
what to do when unbound, every binding names an offer with a known provider, a
secret reaches only the units that declare it, a cloud plan has no
need met by a fallback, and every unit that a unit starts after exists with no
cycle in the start order. It lists the failed checks first, names the unit and
the need of each failure, never prints a value from the plan, and exits with 1
when a check failed or the file is not a plan. It writes nothing and reads no
store or secret store. Each binding and each unbound entry in a plan states
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
