# CI / Non-Interactive Usage

By default, `smuggle install`, `smuggle add`, and `smuggle dev` start a **file watcher that blocks** until you press ctrl-c. They also show **interactive prompts** to select packages.

For scripts, CI pipelines, and non-interactive environments (including LLMs/AI agents), you need to disable both behaviors.

## `--once` — skip the file watcher

Swap packages and exit immediately:

```sh
smuggle publish --all
smuggle install --all --once
```

This is the recommended approach for any automated workflow that just needs to swap packages once.

## `--ci` — full CI mode

The `--ci` flag implies both `--all` and `--once`, plus:

- Emits **NDJSON events** to stdout for machine-readable output
- Writes a **GitHub Actions job summary** when running in GitHub Actions

```sh
smuggle publish --ci
smuggle install --ci
```

## Quick reference

| Scenario | Flags |
|----------|-------|
| Script / automation | `--all --once` |
| CI pipeline | `--ci` |
| AI agent / LLM | `--all --once` |
| Interactive development | _(none, defaults are fine)_ |
