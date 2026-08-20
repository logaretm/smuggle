# Flags

<Callout kind="tip" title="Two you'll use most">

`--all` skips the package selection prompt. `-v` logs every request the proxy handles, not just the ones answered from your local store.

</Callout>

## Global flags

These can be passed to any command, or to `smuggle` with no subcommand.

| Flag                  | Description                                                                 |
| --------------------- | --------------------------------------------------------------------------- |
| `-p, --path <PATH>`   | Path to the project directory (defaults to current directory)               |
| `--all`               | Skip interactive prompts and select all matching packages                   |
| `-v, --verbose`       | Log every request the proxy handles, not only hijacked ones                 |
| `--registry <URL>`    | Intercept an extra registry on top of the ones npm reports. Repeatable      |

Smuggle asks npm which registries a project resolves through, including scoped ones, so `--registry` is only needed for a registry npm is not currently configured to use.

## Command-specific flags

### `smuggle publish`

| Flag                | Description                                                        |
| ------------------- | ------------------------------------------------------------------ |
| `--all`             | Publish all non-private workspace packages without prompting        |
| `-p, --path <PATH>` | Path to the package directory                                      |

### `smuggle unpublish`

| Flag    | Description                       |
| ------- | --------------------------------- |
| `--all` | Remove every registered package   |

### `smuggle proxy`

Mostly for debugging. Sessions start the proxy through the daemon.

| Flag                 | Description                                                            |
| -------------------- | ---------------------------------------------------------------------- |
| `--hijack <PKG>`     | Answer for this package from the local store. Repeatable                |
| `--registry <URL>`   | Registry to intercept. Repeatable                                      |
| `--port <PORT>`      | Port to listen on (default 443)                                        |
| `--listen <IP>`      | Loopback address to listen on (default 127.0.0.2)                      |
| `--no-redirect`      | Listen without editing `/etc/hosts`, so nothing is intercepted         |
| `--verbose`          | Log every request that passes through                                  |
