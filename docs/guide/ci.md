# CI / Non-Interactive Usage

By default, `smuggle install` and `smuggle dev` start a **file watcher that blocks** until you press <kbd>Ctrl</kbd>+<kbd>C</kbd>. They also show **interactive prompts** to select packages.

For scripts, CI pipelines, and non-interactive environments (including LLMs / AI agents), you need to disable both behaviors.

<Callout kind="tip" title="Two flags to remember">

`--once` skips the watcher. `--all` skips the prompts. The combined `--ci` does both, plus emits machine-readable output.

</Callout>

## `--once` &nbsp;skip the watcher

Swap packages and exit immediately:

```sh
smuggle publish --all
smuggle install --all --once
```

Recommended for any automated workflow that just needs to swap packages once.

## `--ci` &nbsp;full CI mode

The `--ci` flag implies both `--all` and `--once`, plus:

- Emits **NDJSON events** to stdout for machine-readable output
- Writes a **GitHub Actions job summary** when running in GitHub Actions

```sh
smuggle publish --ci
smuggle install --ci
```

## GitHub Actions example

```yaml
- name: Smuggle local package
  run: |
    cd packages/my-pkg
    smuggle publish --ci

- name: Run consumer tests with smuggled deps
  run: |
    cd apps/consumer
    smuggle install --ci
    npm test
```

<Callout kind="info" title="Job summary">

When `--ci` runs inside GitHub Actions it appends a markdown table to `$GITHUB_STEP_SUMMARY` listing every package that was packed or swapped — handy for verifying what landed in the run.

</Callout>

## Quick reference

| Scenario                | Flags                              |
| ----------------------- | ---------------------------------- |
| Script / automation     | `--all --once`                     |
| CI pipeline             | `--ci`                             |
| AI agent / LLM          | `--all --once`                     |
| Interactive development | _(none — defaults are fine)_       |

<Callout kind="warn" title="Heads up">

Without `--once`, smuggle keeps a foreground watcher running. CI runners will hang until they hit a step timeout.

</Callout>
