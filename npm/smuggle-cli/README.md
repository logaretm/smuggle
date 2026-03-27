# smuggle

Test local npm packages in real consumer projects — no symlinks, no lockfile pollution, no `.npmrc` hacks.

## The Problem

Testing local packages usually means `npm link`, `pnpm link`, or `file:` references. All of these have tradeoffs:

- **`npm link` / `pnpm link`** — symlinks break pnpm's content-addressable store, cause duplicate dependencies, and behave differently from a real install
- **`file:` / `link:` references** — pollute `package.json` and lockfiles, easy to accidentally commit
- **Local registries (Verdaccio)** — heavyweight setup for a simple dev loop

## The Solution

Smuggle packs your local package the same way `npm publish` would, then directly replaces the installed copy in `node_modules`. Your lockfile and `package.json` stay untouched. When you're done, originals are restored automatically.

```
npm install -g smuggle-cli
```

## Quick Start

**1. Register your local package:**

```sh
cd ~/my-library
smuggle publish
```

**2. Use it in a consumer project:**

```sh
cd ~/my-app
smuggle install
```

That's it. Smuggle will:

- Find registered packages that match your dependencies
- Back up the originals from `node_modules`
- Extract your local build directly into `node_modules`
- Clear bundler caches (Vite, Next.js, webpack)
- Watch for changes and hot-swap on save
- Restore everything on exit (<kbd>Ctrl</kbd>+<kbd>C</kbd>)

## Commands

### `smuggle publish`

Pack and register local packages in `~/.smuggle/packages/`. In a pnpm workspace, you'll be prompted to select which packages to publish.

```sh
smuggle publish        # interactive select
smuggle publish --all  # all non-private packages
```

### `smuggle install`

Install registered packages into the current project. Shorthand: just `smuggle` with no arguments.

```sh
smuggle install
smuggle          # same thing
```

### `smuggle add <package>`

Add a registered package as a dependency — installs it via your package manager and then smuggles the local version in.

```sh
smuggle add @scope/my-pkg
```

### `smuggle list`

List all registered packages.

### `smuggle unpublish <package>`

Remove a package from the local registry.

## How It Works

```
smuggle publish                     smuggle install
  |                                   |
  v                                   v
  Pack files (like npm publish)       Find matches in consumer deps
  |                                   |
  v                                   v
  Store tarball in ~/.smuggle/        Backup node_modules originals
                                      |
                                      v
                                      Extract tarball into node_modules
                                      |
                                      v
                                      Watch source dirs for changes
                                      |
                                      v
                                      On change: re-pack -> hash check
                                      -> extract if changed -> bust caches
                                      |
                                      v
                                      On exit: restore originals
```

## Design

- **No symlinks** — packages are real files in `node_modules`, identical to a normal install
- **No lockfile changes** — `pnpm-lock.yaml`, `package-lock.json`, and `yarn.lock` stay untouched
- **No `.npmrc` changes** — no registry overrides needed
- **No `package.json` changes** — version ranges are preserved
- **Automatic cleanup** — originals are restored on exit, even on <kbd>Ctrl</kbd>+<kbd>C</kbd>
- **Hash-based change detection** — only re-extracts and busts caches when the packed output actually changes
- **Workspace-aware** — detects pnpm workspaces and resolves transitive dependencies

## Supported Package Managers

- pnpm (including workspaces)
- npm
- yarn

## Alternative Installation Methods

### Homebrew

```sh
brew install logaretm/tap/smuggle
```

### Cargo

```sh
cargo install smuggle
```

## Supported Platforms

| Platform | Architecture | Package |
|----------|-------------|---------|
| macOS | Apple Silicon (arm64) | `@smuggle-cli/darwin-arm64` |
| macOS | Intel (x64) | `@smuggle-cli/darwin-x64` |
| Linux | arm64 (glibc) | `@smuggle-cli/linux-arm64-gnu` |
| Linux | x64 (glibc) | `@smuggle-cli/linux-x64-gnu` |

The correct binary is installed automatically based on your platform.

## License

MIT
