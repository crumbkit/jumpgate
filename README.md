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
| `crates/jumpgate` | The record types and their validation. It runs no process and reads no file, secret store or environment variable. |
| `crates/jumpgate-cli` | The `jumpgate` command. |

## Records

| Type | What it holds | Path in a store |
| --- | --- | --- |
| `jumpgate/need/v1` | One thing an app requires. | `/jumpgate/needs/<app>/<name>` |
| `jumpgate/offer/v1` | Something a provider can supply, as a reference. | `/jumpgate/offers/<provider-kind>/<provider>/<name>` |
| `jumpgate/binding/v1` | A need matched to an offer for one target. | `/jumpgate/bindings/<target>/<app>/<need>` |
| `jumpgate/target/v1` | A target and the board whose bindings count for it. | `/jumpgate/targets/<name>` |

Each record's JSON form starts with `"$type"`. The reader refuses an unknown
field, a missing field and a `$type` of another type, and parses every
string into its own type. A refusal carries a code of the form
`jumpgate::record::<reason>` and never quotes the refused text. The JSON
Schemas, draft 2020-12, are in `crates/jumpgate/schemas/jumpgate/`.

## The command

This version holds the record types only. `jumpgate needs`,
`jumpgate resolve` and `jumpgate check` exist and fail with
`jumpgate::command::not_built`. `jumpgate skills install` and
`jumpgate completion install` install the agent skill and the shell
completion.

## Licence

Licensed under either of the Apache License, Version 2.0
([LICENSE-APACHE](LICENSE-APACHE)) or the MIT licence
([LICENSE-MIT](LICENSE-MIT)), at your option.
