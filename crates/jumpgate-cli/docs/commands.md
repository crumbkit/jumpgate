# Commands of `jumpgate`

This reference is generated from the help of `jumpgate`. Edit the help in the tool's code, and not this file.

## `jumpgate`

Match what each app needs to what a provider offers, and check the plan

### Commands

- [`jumpgate needs`](#jumpgate-needs): Read the declarations of a set of repositories and write the need records and the offers each app provides
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

Read the declarations of a set of repositories and write the need records and the offers each app provides.

This version of jumpgate does not build needs yet, and the command fails with jumpgate::command::not\_built.

### Examples

Read the declarations and write the need records (not built yet).

```sh
jumpgate needs
```

Effects: read-only, closed-world.

## `jumpgate resolve`

Match each need to an offer for one target, write the plan and list the needs that nothing binds.

This version of jumpgate does not build resolve yet, and the command fails with jumpgate::command::not\_built.

### Examples

Match needs to offers and write the plan (not built yet).

```sh
jumpgate resolve
```

Effects: read-only, closed-world.

## `jumpgate check`

Run every check on a plan, whichever matcher wrote it.

This version of jumpgate does not build check yet, and the command fails with jumpgate::command::not\_built.

### Examples

Run every check on a plan (not built yet).

```sh
jumpgate check
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

- [`jumpgate skills install`](#jumpgate-skills-install): Install jumpgate's skill into each agent skills directory that exists
- [`jumpgate skills check`](#jumpgate-skills-check): Check that each installed copy of jumpgate's skill matches this jumpgate
- [`jumpgate skills print`](#jumpgate-skills-print): Print jumpgate's skill

## `jumpgate skills install`

Install jumpgate's skill into each agent skills directory that exists

### Arguments

- `--dir <DIR>`: Install into this skills directory in place of the agent skills directories
- `--force`: Replace a file or a link that install did not write with the skill's own folder

### Examples

Install jumpgate's skill into each agent skills directory that exists.

```sh
jumpgate skills install
```

Install it, replacing a file or a link that install did not write.

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

Check that each installed copy of jumpgate's skill matches this jumpgate

### Arguments

- `--dir <DIR>`: Check this skills directory in place of the agent skills directories

### Examples

Report each agent skills directory whose copy is missing or differs.

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
- `reason`: Why install skipped or failed, or what it replaced under --force, such as `linked to /src/shelf/shelf.fish`, or null.

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
- `reason`: Why the file differs, cannot be read or is linked, such as `linked to /src/shelf/shelf.fish`, or null.

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
