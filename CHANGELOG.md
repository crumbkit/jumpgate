# Changelog

This file records the changes to jumpgate that a workspace may need to act on.

## 0.1.0

### Added

- `jumpgate needs --repo <path>...` reads the `mxv.toml` and `starmap.toml`
  files of each checkout and writes the needs, provides and units they give
  to a store. It replaces the records of each app it reads without a refusal
  and keeps the records of an app whose declaration it refuses, which it
  reports with a code, file and line.
- `jumpgate resolve --target <name>` matches the needs in the store to its
  provides for one target and writes the plan, `jumpgate/plan/v1alpha1`, which
  holds references and never a secret value. It lists the needs that nothing
  binds before the units, and exits with 1 when a need that fails when
  unbound has no provide.
- `jumpgate check <plan>` reads one plan, whichever matcher wrote it, and
  runs six checks on it. Unless `--plan-only` is given, it also compares the
  plan with the store.
- The default store is the personal board, read and written through the
  `board` command. `--board <dir>` names another board, and `--store <dir>`
  names a directory of JSON files, which works with no board.
- An app declares what it requires in `[needs.<name>]` and what it supplies in
  `[provides.<name>]` of `mxv.toml`. The first kind of provide is `socket`,
  and `resolve` binds a need of that name to the stored socket path under the
  checkout.
- Each `[service.<name>]` of `mxv.toml` that a process lists in `needs` gives
  a need for its endpoint variable and a provide of the workspace. The matcher
  refuses to bind a service need to a provide of another kind, and a service
  need on a cloud target with no binding record is stated as `local_only` in
  the plan.
- A process in `mxv.toml` can name the `starmap.toml` it reads with an
  optional `starmap` key, as a path from the root of the checkout. A named
  file that the checkout lacks is refused with
  `jumpgate::declaration::starmap_missing`. `jumpgate needs` lists each
  git-tracked `starmap.toml` that no process reads under "No process reads:".
- `jumpgate skills install` and `jumpgate completion install` install the
  agent skill and the shell completion.
