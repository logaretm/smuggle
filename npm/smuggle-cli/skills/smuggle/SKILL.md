---
name: smuggle
description: Use when the user wants to test a local npm package in a consumer project, replace node_modules with a local build, hot-reload local packages, or avoid npm link / symlink issues. Also use when the user mentions "smuggle", "local package testing", or asks how to test a package they're developing locally.
allowed-tools: Bash, Read, Glob, Grep
---

# Smuggle — Local npm Package Testing

Smuggle lets you test local npm packages in real consumer projects by directly overwriting files in `node_modules`. No symlinks, no lockfile pollution, no `.npmrc` hacks. Originals are backed up and restored on exit.

## Prerequisites

Check if smuggle is installed:

```sh
smuggle --help
```

If not installed, install it with one of:

```sh
npm install -g smuggle-cli   # via npm
brew install logaretm/tap/smuggle   # via Homebrew
cargo install smuggle   # via Cargo
```

## Commands

### `smuggle publish` — Register local packages

Run this inside the package you're developing (or a pnpm workspace root). It packs the package the same way `npm publish` would and stores the tarball in `~/.smuggle/packages/`.

```sh
# Interactive — select which packages to publish
smuggle publish

# Publish all non-private packages in a workspace
smuggle publish --all
```

**When to use:** The user has a library/package they're working on and wants to make it available for testing in another project.

### `smuggle install` — Install into a consumer project

Run this inside the project that depends on the package. Can also be invoked as just `smuggle` with no arguments.

```sh
smuggle install
# or just:
smuggle
```

This will:

1. Scan `package.json` dependencies for matches against registered packages
2. Prompt to select which ones to smuggle in
3. Auto-include transitive dependencies that are also registered
4. Back up the original `node_modules` copies
5. Extract local packages directly into `node_modules`
6. Clear bundler caches (Vite, Next.js, webpack) and touch `vite.config.*` to trigger a restart
7. Watch for source changes — on change, re-pack and re-extract instantly
8. Restore everything on exit (Ctrl+C)

**When to use:** The user wants to test their local package changes inside a real consumer project.

### `smuggle add <package>` — Add a package as a dependency

Installs the package via the project's package manager (npm/pnpm/yarn) and then smuggles the local version in. Useful when the consumer project doesn't depend on the package yet.

```sh
smuggle add @scope/my-pkg
```

**When to use:** The user wants to add a new dependency AND test the local version of it.

### `smuggle list` — List registered packages

```sh
smuggle list
```

Shows all packages currently registered in `~/.smuggle/packages/`.

### `smuggle unpublish <package>` — Remove a registered package

```sh
smuggle unpublish @scope/my-pkg
```

## Typical Workflow

Here's the standard two-terminal workflow:

**Terminal 1 — the library being developed:**

```sh
cd ~/my-library
smuggle publish
# Edit code... re-run publish when ready, or use install --watch from the consumer side
```

**Terminal 2 — the consumer project:**

```sh
cd ~/my-app
smuggle install
# Smuggle watches for changes and hot-swaps automatically
# Press Ctrl+C to restore originals and exit
```

## How It Works

- Packages are packed into tarballs identical to `npm publish` output
- Tarballs are stored in `~/.smuggle/packages/` indexed by package name
- On install, the original `node_modules` contents are backed up to a temp directory
- The local tarball is extracted directly into `node_modules`, replacing the installed version
- File watching uses hash-based change detection — cache busting and re-extraction only happen when the packed output actually changes
- On exit, originals are restored from the backup

## Key Design Decisions

- **No symlinks** — packages are real files, behaving identically to a normal install
- **No lockfile changes** — `pnpm-lock.yaml`, `package-lock.json`, `yarn.lock` stay untouched
- **No `.npmrc` changes** — no registry overrides
- **No `package.json` changes** — version ranges are preserved
- **Workspace-aware** — detects pnpm workspaces and resolves transitive dependencies across workspace members
- **Automatic cleanup** — originals are always restored, even on Ctrl+C

## Supported Package Managers

- pnpm (including workspaces)
- npm
- yarn

## Troubleshooting

- **"No matching packages found"** — run `smuggle list` to verify the package is registered, and check that the consumer's `package.json` lists it as a dependency
- **Changes not reflecting** — smuggle uses hash-based detection; make sure the source files that are included in the package (per the `files` field in `package.json`) have actually changed
- **Bundler not restarting** — smuggle busts caches for Vite, Next.js, and webpack; for other bundlers, you may need to restart manually
