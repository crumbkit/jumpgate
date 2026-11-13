---
name: jumpgate
description: Match what each app needs to what a provider offers, and check the plan that holds no secret value. Use when an app's wants, secrets or bindings need to be read, matched or checked.
---

# Matching needs to offers with jumpgate

jumpgate reads what each app needs and what each provider offers, matches
them for one target, and writes a plan that holds references and never a
secret value.

## Reading declarations

`jumpgate needs --repo <path> [--repo <path>...] --store <dir>` reads the
declarations of each checkout and writes one JSON file for each need, offer
and unit under `<dir>/jumpgate/`. It reads `mxv.toml` at the checkout root and
each `starmap.toml` that a process's `cwd` names, and runs no process but
`git remote get-url origin`.

- A repository's id is its `origin` URL as `host/owner/name`. Pass
  `--repo <host/owner/name>=<path>` for a checkout with no usable `origin`.
- Each `[wants.<name>]` table of `mxv.toml` gives a need. Each variable of a
  `starmap.toml` with `source = "secret"` gives a secret need, and a `bind`
  also gives an offer that holds the reference, never the value.
- A refused declaration prints its code, file and line, still writes the
  records of everything else, and makes the command exit with 1. Read the
  failures on standard error, or under `failures` with `--json`.
- Without `--store` the command fails with `jumpgate::command::store_required`,
  because the personal board is not built yet.

`jumpgate resolve` and `jumpgate check` exist and fail with
`jumpgate::command::not_built`, which has no next step.

Run `jumpgate commands list` to see every command, and
`jumpgate schemas list` to see the documents jumpgate prints.
