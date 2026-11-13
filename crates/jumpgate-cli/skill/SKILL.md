---
name: jumpgate
description: Match what each app needs to what a provider offers, and check the plan that holds no secret value. Use when an app's wants, secrets or bindings need to be read, matched or checked.
---

# Matching needs to offers with jumpgate

jumpgate reads what each app needs and what each provider offers, matches
them for one target, and writes a plan that holds references and never a
secret value.

This version of jumpgate holds the record types only. `jumpgate needs`,
`jumpgate resolve` and `jumpgate check` exist and fail with
`jumpgate::command::not_built`, which has no next step.

Run `jumpgate commands list` to see every command, and
`jumpgate schemas list` to see the documents jumpgate prints.
