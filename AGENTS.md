# Working on jumpgate

jumpgate is a Cargo workspace with a library, `crates/jumpgate`, which runs
no process, and the `jumpgate` command, `crates/jumpgate-cli`. The rules are
the Rust standards that [rust-standards.toml](rust-standards.toml) adopts.
A change is ready when every step of the `lane` job in
[.github/workflows/ci.yml](.github/workflows/ci.yml) passes locally.

- Write the test first, named as a sentence, and give each refusal a twin
  that shows the accepted case.
- A record type's JSON Schema is committed under
  `crates/jumpgate/schemas/`. After changing a record type, run
  `JUMPGATE_SCHEMAS=overwrite cargo nextest run -p jumpgate --test schemas`
  and review the diff.
- Fixtures use invented apps, repositories and references, such as `shelf`,
  `github.com/acme/shelf` and `op://shelf-dev/api-token/credential`. No
  test reads a secret store.
- The repository is public in style: apart from the licences, no file or
  commit refers to the owner, and none names a project outside crumbkit.

## Writing

Docs, comments and commit messages use plain declarative sentences with a
named subject. Avoid "not X but Y", fragments and closing flourishes.
Commit messages describe changed behaviour.
