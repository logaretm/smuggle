# AGENTS.md

## What is smuggle?

A CLI tool that lets you test local npm packages in consumer projects by directly overwriting files in `node_modules`. No symlinks, no lockfile changes, no `.npmrc` changes. Originals are backed up and restored on exit.

## Architecture

- `src/main.rs` — CLI entry point, install/publish/watch flows
- `src/pack.rs` — Tarball packing (like `npm pack`) and extraction
- `src/store.rs` — Local package store at `~/.smuggle/packages/`
- `src/workspace.rs` — pnpm workspace detection and package scanning
- `src/pm.rs` — Bundler cache clearing (vite, next.js, webpack)

## Core design principles

- **Zero pollution**: Never modify `package.json`, lockfiles, or `.npmrc`. The consumer project should look untouched after smuggle exits.
- **Direct overwrite**: Packages are extracted straight into `node_modules`, resolving symlinks for pnpm. No registry server, no package manager install cycle.
- **Backup/restore**: Original `node_modules` contents are backed up to a temp dir before overwriting and restored on exit (both ctrl-c and normal).
- **Hash-based change detection**: On watch, tarballs are hashed before/after repacking. Cache busting and vite restarts only happen when content actually changes.

## Build, test, format, and lint

```sh
cargo build
cargo test
cargo install --path .

# Format code
cargo fmt

# Check formatting (CI uses this)
cargo fmt -- --check

# Lint with clippy (CI treats warnings as errors)
cargo clippy -- -D warnings
```

Tests use the `@test-smug/` scope to avoid conflicts. Integration tests that need a package manager check for availability and skip if not found.

## Key conventions

- UI uses `cliclack` for prompts, spinners, and log messages. Do not use `println!` or `eprintln!` directly for user-facing output.
- Console styling uses the `console` crate's `style()`.
- The `files` field in `package.json` may contain negation patterns (`!path`) — these must be skipped, not treated as paths.
- pnpm hard-links files from a global store. When overwriting, remove the file first to break the hard link, then write new content.
- Workspace support: search all member `node_modules/` directories, deduplicate by resolved (canonicalized) path.

## Release flow

Releases are driven by changesets. See `.changeset/README.md` for the details.

1. Every PR adds a changeset (`pnpm changeset`, or `pnpm changeset --empty` for
   anything not user-visible). CI fails the PR without one.
2. Merging a changeset to `main` opens or updates a `chore: release` PR holding
   the version bump and the CHANGELOG entry. Pending changesets accumulate into
   that one PR.
3. Merging the release PR is the release. "Publish" tags `vX.Y.Z` and calls
   "Release", which builds the four platform binaries, publishes them and
   `smuggle-cli` to npm, and cuts a GitHub release from the new CHANGELOG
   section.
4. After release, update Homebrew tap: `gh workflow run update-formula.yml -R logaretm/homebrew-tap -f formula=smuggle -f version=X.Y.Z -f repo=logaretm/smuggle`

Changesets versions `npm/smuggle-cli` and `scripts/sync-versions.ts` copies that
version onto `Cargo.toml`, `Cargo.lock`, and the `@smuggle-cli/*` platform
packages. Never hand-edit a version number.
