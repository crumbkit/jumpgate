# Commands of `jumpgate`

This reference is generated from the help of `jumpgate`. Edit the help in the tool's code, and not this file.

## `jumpgate`

Match what each app needs to what a provider offers, and check the plan

### Commands

- [`jumpgate needs`](#jumpgate-needs): Read the declarations of a set of repositories and write the need records, the offers each app provides and the units each app runs
- [`jumpgate resolve`](#jumpgate-resolve): Match each need to an offer for one target, write the plan and list the needs that nothing binds
- [`jumpgate check`](#jumpgate-check): Run every check on a plan, whichever matcher wrote it
- [`jumpgate commands`](#jumpgate-commands): Explore jumpgate's commands
- [`jumpgate schemas`](#jumpgate-schemas): Explore the JSON documents that jumpgate prints
- [`jumpgate skills`](#jumpgate-skills): Give coding agents jumpgate's skill
- [`jumpgate completion`](#jumpgate-completion): Set up jumpgate's completion in a shell

### Arguments

- `--json`: Print the result as a JSON document. The default prints text. Works after every subcommand.
- `--fields <FIELD>`: Print only these comma-separated fields, in this order. The default prints the compact columns. Takes more than one value. Works after every subcommand.
- `--long`: Add the long columns after the compact ones. The default prints only the compact columns. Works after every subcommand.
- `--full`: With --json, print every field of every row. The default prints only the fields of the columns. Works after every subcommand.
- `--no-input`: Fail instead of asking a question. Works after every subcommand.

## `jumpgate needs`

Read the declarations of a set of repositories and write the need records, the offers each app provides and the units each app runs.

It reads `mxv.toml` and the `starmap.toml` files that its processes read in each checkout, and runs no process but `git remote get-url origin` and `board`. The records go to the personal board unless `--store` names a directory or `--board` names another board. A declaration it refuses is reported and exits with 1, and the records of everything else are still written. It deletes the records that an app it read without a refusal no longer declares.

### Arguments

- `--repo <REPO>`: A checkout to read, as a path or as `<host/owner/name>=<path>`. The repository's id is the normalised URL of the checkout's git remote `origin` unless the value gives one. Repeat the option for each repository of the stack. A comma is part of a path, so one use names one checkout. Required. Takes more than one value.
- `--store <DIR>`: A directory of JSON files, one for each record, to use as the store instead of a board. The command creates it when it writes and it is missing
- `--board <DIR>`: The directory of the board to use as the store. Without `--store` and `--board` the store is the personal board

### Examples

Read the declarations of two checkouts and write their needs, offers and units to ./records.

```sh
jumpgate needs --repo ../shelf --repo ../auth --store ./records
```

Write them to the personal board.

```sh
jumpgate needs --repo ../shelf --repo ../auth
```

Effects: additive, idempotent, closed-world.

## `jumpgate resolve`

Match each need to an offer for one target, write the plan and list the needs that nothing binds.

It reads the target, needs, offers, units and bindings in a store, the personal board unless `--store` or `--board` names another, and runs no process but `board`. A need that nothing meets, and an ambiguous match, is listed before the bound needs and makes the command exit with 1 after it writes the plan. The plan holds references and no secret value.

### Arguments

- `--target <TARGET>`: The name of the target to resolve for, which the store holds as a `jumpgate/target/v1` record. Required.
- `--store <DIR>`: A directory of JSON files, one for each record, to use as the store instead of a board. The command creates it when it writes and it is missing
- `--board <DIR>`: The directory of the board to use as the store. Without `--store` and `--board` the store is the personal board
- `--out <FILE>`: The file to write the plan to, through a temporary file and a rename. Without it, the command prints the plan as its output

### Examples

Match the needs in ./records to offers for the target local and write the plan to plan.json.

```sh
jumpgate resolve --target local --store ./records --out plan.json
```

Print the plan for the target local from the board in ../team-board.

```sh
jumpgate resolve --target local --board ../team-board
```

Effects: additive, idempotent, closed-world.

## `jumpgate check`

Run every check on a plan, whichever matcher wrote it.

It reads the plan file and the store the plan came from, the personal board unless `--store` or `--board` names another, and writes nothing. It reads no secret store and resolves no reference. Each unit's target, each need's kind and consumers, and each binding's need and offer must exist in the store and agree with the plan, and `--plan-only` skips that comparison. A plan that does not parse is refused with its parse code and no check runs. Otherwise every check runs, the failed checks are listed before the passed ones, and the command exits with 1 when a check failed. The checks are that the plan holds no secret value, every need is bound or has a stated when\_unbound, every binding names an offer with a known provider, a secret reaches only the units that declare it, a cloud plan has no need met by a fallback, and every unit that a unit starts after exists with no cycle in the start order.

### Arguments

- `<PLAN>`: The plan file to check, as `jumpgate resolve --out` wrote it, or as another matcher or a person wrote it. Required.
- `--store <DIR>`: A directory of JSON files, one for each record, to use as the store instead of a board. The command creates it when it writes and it is missing
- `--board <DIR>`: The directory of the board to use as the store. Without `--store` and `--board` the store is the personal board
- `--plan-only`: Check the plan alone and read no store, so the command works with no board to read

### Examples

Run the six checks on plan.json, compare it with the personal board and list the failed checks first.

```sh
jumpgate check plan.json
```

Run the six checks on plan.json alone and read no board.

```sh
jumpgate check plan.json --plan-only
```

Effects: read-only, closed-world.

## `jumpgate commands`

Explore jumpgate's commands

### Commands

- [`jumpgate commands list`](#jumpgate-commands-list): List a branch of jumpgate's commands with their arguments, examples and columns

## `jumpgate commands list`

List a branch of jumpgate's commands with their arguments, examples and columns

### Arguments

- `<PATH>`: The names of the commands below the root that lead to the branch, such as `books list`. Takes more than one value.
- `--limit <N>`: Print at most this many rows. Default: `25`.
- `--all`: Print every row

### Examples

List every command.

```sh
jumpgate commands list
```

### Columns

- `path`: The command line that runs the command, such as `shelf books list`.
- `about`: The command's one-line description, or its full description when it has no one-line description.

Effects: read-only, closed-world.

## `jumpgate schemas`

Explore the JSON documents that jumpgate prints

### Commands

- [`jumpgate schemas list`](#jumpgate-schemas-list): List the JSON documents that jumpgate prints, with a description of each
- [`jumpgate schemas show`](#jumpgate-schemas-show): Print the JSON Schema of each document

## `jumpgate schemas list`

List the JSON documents that jumpgate prints, with a description of each

### Arguments

- `--limit <N>`: Print at most this many rows. Default: `25`.
- `--all`: Print every row

### Examples

List every document that jumpgate prints.

```sh
jumpgate schemas list
```

### Columns

- `id`: The document's id, such as `shelf/books-list/v1`, which `schemas show` takes.
- `kind`: output when a command prints the document with its schema id, unwrapped when a command prints it without the envelope that holds its schema id, and file when it is a file format that users write and no command prints.
- `form`: stream when a command prints the document as a stream, one document on each line as each row arrives, and document when a command prints it once or it is a file format.
- `description`: What the document holds, in one line.

### Columns that `--long` adds

- `commands`: The command lines that print the document. It is empty for the failure document, which every command prints when it fails with --json, and for a file format.

Effects: read-only, closed-world.

## `jumpgate schemas show`

Print the JSON Schema of each document

### Arguments

- `<ID>`: The ids of the documents that `schemas list` lists, separated by spaces. Required. Takes more than one value.

### Examples

Print the JSON Schema of the document that schemas list prints.

```sh
jumpgate schemas show jumpgate/schemas-list/v1
```

### Columns

- `id`: The document's id, such as `shelf/books-list/v1`.
- `schema`: The document's 2020-12 JSON Schema.

Effects: read-only, closed-world.

## `jumpgate skills`

Give coding agents jumpgate's skill

### Commands

- [`jumpgate skills install`](#jumpgate-skills-install): Install jumpgate's skills into each agent skills directory that exists
- [`jumpgate skills check`](#jumpgate-skills-check): Check that each installed copy of jumpgate's skills matches this jumpgate
- [`jumpgate skills print`](#jumpgate-skills-print): Print jumpgate's skill

## `jumpgate skills install`

Install jumpgate's skills into each agent skills directory that exists

### Arguments

- `--dir <DIR>`: Install into this skills directory in place of the agent skills directories
- `--force`: Replace a file or a link that install did not write with the skill's own folder

### Examples

Install jumpgate's skills into each agent skills directory that exists.

```sh
jumpgate skills install
```

Install jumpgate's skills, replacing a file or a link that install did not write.

```sh
jumpgate skills install --force
```

### Columns

- `state`: What install did: failed, skipped, written or unchanged.
- `skill`: The skill's name, which names its folder in each skills directory, such as `shelf`.
- `dirs`: The skills directories that share this copy, as named. Directories that resolve to one place through a symbolic link share one copy.
- `reason`: Why install skipped or failed, or what it replaced under --force, such as `linked to /src/shelf/skill`, or null.

### Columns that `--long` adds

- `path`: The SKILL.md file that install wrote or left, or the skill's other file whose write failed, at its real path.

Effects: additive, idempotent, closed-world.

## `jumpgate skills check`

Check that each installed copy of jumpgate's skills matches this jumpgate

### Arguments

- `--dir <DIR>`: Check this skills directory in place of the agent skills directories

### Examples

Report each agent skills directory where jumpgate's skills are missing or differ.

```sh
jumpgate skills check
```

### Columns

- `state`: Whether the copy is the binary's: missing, differs, unreadable, linked or current.
- `skill`: The skill's name, which names its folder in each skills directory, such as `shelf`.
- `dirs`: The skills directories that share this copy, as named. Directories that resolve to one place through a symbolic link share one copy.
- `reason`: Why the copy is missing a file, differs, cannot be read or is linked, such as `written by shelf 0.2.0`, `REFERENCE.md is missing` or `linked to /src/shelf/skill`, or null.

### Columns that `--long` adds

- `path`: The SKILL.md file that was checked, at its real path.

Effects: read-only, closed-world.

## `jumpgate skills print`

Print jumpgate's skill

### Examples

Print jumpgate's skill.

```sh
jumpgate skills print
```

Effects: read-only, closed-world.

## `jumpgate completion`

Set up jumpgate's completion in a shell

### Commands

- [`jumpgate completion install`](#jumpgate-completion-install): Install jumpgate's completion where the shell loads it
- [`jumpgate completion check`](#jumpgate-completion-check): Check that jumpgate's installed completion file matches this jumpgate
- [`jumpgate completion print`](#jumpgate-completion-print): Print jumpgate's completion registration script for a shell

## `jumpgate completion install`

Install jumpgate's completion where the shell loads it

### Arguments

- `--shell <SHELL>`: Install for this shell in place of the one that SHELL names. One of `bash`, `elvish`, `fish`, `powershell`, `zsh`.
- `--force`: Replace a file or a link that install did not write; a link's target stays as it is

### Examples

Install jumpgate's completion for fish.

```sh
jumpgate completion install --shell fish
```

Install it for zsh, replacing a file or a link that install did not write.

```sh
jumpgate completion install --shell zsh --force
```

### Columns

- `state`: What install did: failed, skipped, written or unchanged.
- `shell`: The shell: bash, elvish, fish, powershell or zsh.
- `path`: The completion file that install wrote or left. When the file is a link to a file that install wrote, the path of that file.
- `reason`: Why install skipped or failed, or what it replaced under --force, such as `linked to /path/to/target.fish`, or null.

Effects: additive, idempotent, closed-world.

## `jumpgate completion check`

Check that jumpgate's installed completion file matches this jumpgate

### Arguments

- `--shell <SHELL>`: Check the file of this shell in place of the one that SHELL names. One of `bash`, `elvish`, `fish`, `powershell`, `zsh`.

### Examples

Report zsh's completion file when it is missing or differs.

```sh
jumpgate completion check --shell zsh
```

### Columns

- `state`: Whether the file holds the script that install writes: missing, differs, unreadable, linked or current.
- `shell`: The shell: bash, elvish, fish, powershell or zsh.
- `path`: The completion file that was checked. When the file is a link to a file that install wrote, the path of that file.
- `reason`: Why the file differs, cannot be read or is linked, such as `linked to /path/to/target.fish`, or null.

Effects: read-only, closed-world.

## `jumpgate completion print`

Print jumpgate's completion registration script for a shell

### Arguments

- `--shell <SHELL>`: Print the script of this shell in place of the one that SHELL names. One of `bash`, `elvish`, `fish`, `powershell`, `zsh`.

### Examples

Print the registration script for fish.

```sh
jumpgate completion print --shell fish
```

Effects: read-only, closed-world.
