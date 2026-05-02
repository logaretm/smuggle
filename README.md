<p align="center">
  <img src="docs/public/logo.svg" alt="smuggle" width="128" height="128" />
</p>

<h1 align="center">smuggle</h1>

<p align="center">Smuggle local npm packages into your projects — no symlinks, no lockfile pollution.</p>

## Why?

Testing local packages in a real consumer project usually means `npm link`, `pnpm link`, or `file:` references. All of these pollute your lockfile, break with pnpm's content-addressable store, or behave differently from a real install.

Smuggle takes a different approach: it packs your local package the same way `npm publish` would, then directly replaces the installed copy in `node_modules`. Your lockfile and `package.json` stay untouched. When you're done, originals are restored automatically.

## Install

### npm / pnpm / yarn

```sh
npm install -g smuggle-cli
# or
pnpm add -g smuggle-cli
# or
yarn global add smuggle-cli
```

### Homebrew

```sh
brew install logaretm/tap/smuggle
```

### Cargo

```sh
cargo install smuggle
```

### From source

```sh
git clone https://github.com/logaretm/smuggle.git
cd smuggle
cargo install --path .
```

## Usage

| Command | Description |
|---------|-------------|
| `smuggle publish` | Pack and register local packages for later use |
| `smuggle` or `smuggle install` | Swap registered packages into a consumer project and watch for changes |
| `smuggle add <pkg>` | Add an unreleased package to your dependencies and swap it in |
| `smuggle dev` | Swap packages and run your dev server |
| `smuggle list` | List all registered packages |
| `smuggle unpublish <pkg>` | Remove a registered package |

### `smuggle publish`

In your package directory (or workspace root):

```sh
smuggle publish
```

This packs the package and registers it locally in `~/.smuggle/packages/`. In a pnpm workspace, you'll be prompted to select which packages to publish.

Use `--all` to skip the prompt and publish all non-private packages:

```sh
smuggle publish --all
```

### `smuggle install`

In your consumer project directory:

```sh
smuggle install
```

Or just:

```sh
smuggle
```

This will:

1. Find registered packages that match the consumer's dependencies
2. Let you select which ones to proxy
3. Auto-include transitive dependencies that are also registered
4. Back up the originals from `node_modules`
5. Extract your local packages directly into `node_modules`
6. Clear bundler caches (Vite, Next.js, webpack) and touch `vite.config.*` to trigger a restart
7. Watch for changes in the source packages — on change, re-pack and re-extract instantly
8. Restore everything on exit (ctrl-c)

### `smuggle add`

```sh
smuggle add @scope/my-pkg
# or multiple at once
smuggle add @scope/pkg-a @scope/pkg-b
```

This adds the package(s) to your `package.json` dependencies (resolving transitive deps via your package manager), then swaps them with the local versions. Use `--dev` to add as devDependencies instead.

> **Note:** `smuggle add` is mainly useful for packages that haven't been released yet and aren't in your `package.json`. If the package is already a dependency, just run `smuggle` with no subcommand.

### `smuggle dev`

```sh
smuggle dev
```

This combines `smuggle install` with your dev server — it swaps packages, starts the dev server (auto-detected from your `package.json` "dev" script), and watches for changes. You can pass a custom command:

```sh
smuggle dev -- npm run start
```

Use `--restart` to kill and restart the dev server on each package change instead of relying on HMR.

### `smuggle list`

```sh
smuggle list
```

### `smuggle unpublish`

```sh
smuggle unpublish @scope/my-pkg
```

## Flags

### Global flags

These flags can be passed to any command (or to `smuggle` with no subcommand):

| Flag | Description |
|------|-------------|
| `-p, --path <PATH>` | Path to the project directory (defaults to current directory) |
| `--all` | Select all matching packages without prompting |
| `--once` | Swap packages once and exit without watching for changes |
| `--ci` | CI mode: implies `--all --once`, emits NDJSON events, writes GitHub Actions summary |

### Command-specific flags

| Command | Flag | Description |
|---------|------|-------------|
| `add` | `--dev` | Add as a devDependency instead of a dependency |
| `dev` | `--restart` | Kill and restart the dev server on each package change (instead of relying on HMR) |

## How it works

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

Key design decisions:

- **No symlinks** — packages are real files in `node_modules`, just like a normal install
- **No lockfile changes** — your `pnpm-lock.yaml` / `package-lock.json` / `yarn.lock` stays untouched
- **No `.npmrc` changes** — no registry overrides needed
- **No `package.json` changes** — version ranges are preserved (`smuggle install`; `smuggle add` does modify `package.json` to add the dependency)
- **Automatic cleanup** — originals are restored on exit, even on ctrl-c
- **Hash-based change detection** — only triggers cache busting and Vite restarts when the packed output actually changes
- **Workspace support** — detects pnpm workspaces and scans all member packages for matching dependencies

## Supported package managers

- pnpm (including workspaces)
- npm
- yarn

## Development

This project uses [just](https://github.com/casey/just) as a command runner. Run all CI checks locally:

```sh
just check
```

Or run individual steps:

```sh
just fmt    # check formatting
just lint   # clippy lints
just build  # compile
just test   # tests
```

You'll need the `rustfmt` and `clippy` components (included with `rustup` by default). The CI also expects Node.js and pnpm to be available for integration tests.

## License

MIT
