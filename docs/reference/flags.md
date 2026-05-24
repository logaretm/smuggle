# Flags

<Callout kind="tip" title="Three you'll use most">

`--all` skips selection prompts. `--once` skips the file watcher. `--ci` does both, plus emits machine-readable output.

</Callout>

## Global flags

These flags can be passed to any command or to `smuggle` with no subcommand.

| Flag                | Description                                                                                       |
| ------------------- | ------------------------------------------------------------------------------------------------- |
| `-p, --path <PATH>` | Path to the project directory (defaults to current directory)                                     |
| `--all`             | Skip interactive prompts and select all matching packages                                         |
| `--once`            | Swap packages and exit immediately (don't start the file watcher)                                 |
| `--ci`              | CI mode: implies `--all --once`, emits NDJSON events to stdout, writes GitHub Actions job summary |

## Command-specific flags

### `smuggle install`

| Flag                | Description                                                            |
| ------------------- | ---------------------------------------------------------------------- |
| `--dev`             | Add new packages as devDependencies instead of dependencies            |
| `--all`             | Skip package selection prompt                                          |
| `--once`            | Don't start the file watcher after installing                          |
| `-p, --path <PATH>` | Path to the consumer project                                          |

### `smuggle publish`

| Flag                | Description                                                            |
| ------------------- | ---------------------------------------------------------------------- |
| `--all`             | Publish all non-private workspace packages (skip interactive selection) |
| `-p, --path <PATH>` | Path to the package directory                                          |

### `smuggle dev`

| Flag                | Description                                                                            |
| ------------------- | -------------------------------------------------------------------------------------- |
| `--restart`         | Kill and restart the dev server on each package change (instead of relying on HMR)     |
| `--all`             | Skip package selection prompt                                                          |
| `-p, --path <PATH>` | Path to the consumer project                                                           |

### `smuggle unpublish`

| Flag    | Description                                                  |
| ------- | ------------------------------------------------------------ |
| `--all` | Remove all registered packages (skip interactive selection)  |
