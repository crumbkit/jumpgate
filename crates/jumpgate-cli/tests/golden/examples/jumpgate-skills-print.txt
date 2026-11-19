---
name: jumpgate
description: Match what each app needs to what a provider provides, and check the plan that holds no secret value. Use when an app's needs, secrets or bindings need to be read, matched or checked.
---

# Matching needs to provides with jumpgate

jumpgate reads what each app needs and what each provider provides, matches
them for one target, and writes a plan that holds references and never a
secret value.

## Reading declarations

`jumpgate needs --repo <path> [--repo <path>...]` reads the declarations of
each checkout and writes each need, provide and unit to the store. It reads
`mxv.toml` at the checkout root and each `starmap.toml` that a process's `cwd`
names, and runs no process but `git remote get-url origin` and `board`.

- A repository's id is its `origin` URL as `host/owner/name`. Pass
  `--repo <host/owner/name>=<path>` for a checkout with no usable `origin`.
- Each `[service.<name>]` table of `mxv.toml` that a process lists in `needs`
  gives a need for the service's endpoint variable and the keys of its `env`,
  and a provide `workspace/mxv/<name>`. A service no process lists is printed
  under `unused`. A process that lists an undeclared service is refused with
  `jumpgate::declaration::service`.
- Each `[needs.<name>]` table of `mxv.toml` gives a need. Each variable of a
  `starmap.toml` with `source = "secret"` gives a secret need, and a `bind`
  also gives a provide that holds the reference, never the value.
- A refused declaration prints its code, file and line, still writes the
  records of everything else, and makes the command exit with 1. Read the
  failures on standard error, or under `failures` with `--json`.
- The command deletes the records of a need, provide or unit that an app it read
  without a refusal no longer declares. It leaves the records of other apps,
  of an app whose declaration is refused, and every binding and target.
- Without `--store` the command writes to the personal board, through the
  `board` command. Pass `--board <dir>` for another board, or `--store <dir>`
  for a directory of JSON files. A board that cannot be read fails with
  `jumpgate::store::board`.

## Matching for a target

`jumpgate resolve --target <name> [--store <dir> | --board <dir>] [--out <file>]`
reads the target record `/jumpgate/targets/<name>` and every need, provide,
unit and binding in the store, and writes the plan, `jumpgate/plan/v1alpha1`,
to `--out` or to standard output. The plan holds references and fallback
values and never a secret value.

- The plan lists unbound needs first. A need that fails when unbound, a
  binding to a missing provide, two provides that meet one need and a fallback
  on a cloud target each make the command exit with 1 after it writes the
  plan, with the code `jumpgate::need::unbound`, `provide_missing`, `ambiguous`
  or `fallback_on_cloud`. A service need on a cloud target is unbound with
  `jumpgate::need::local_only`, because only a workspace supplies the
  service. An optional need that nothing provides is listed and does not
  block.
- A binding record counts only from the board that the target names as its
  authority. The report notes a binding from another board and ignores it.
- A missing target fails with `jumpgate::target::missing`.

`jumpgate check PLAN` reads a plan file and runs six checks: no secret value,
every need bound or stated when unbound, known providers, secret reach, no
cloud fallback and a valid start order. It lists the failed checks first with
the code `jumpgate::check::<reason>`, the unit and the need, and exits with 1
when one failed. A file that is not a plan is refused with its
`jumpgate::record::*` code. It also compares the plan with the store, the
personal board by default: the target, each need's kind and consumers and
each binding's need and provide must exist and agree, with codes such as
`jumpgate::check::target_unknown` and `need_omitted`. `--plan-only` skips the
comparison. It writes nothing and never prints a value.

Run `jumpgate commands list` to see every command, and
`jumpgate schemas list` to see the documents jumpgate prints.
