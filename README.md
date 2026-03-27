# smuggle

Smuggle local npm packages into your projects — no symlinks, no lockfile pollution.

## Why?

Testing local packages in a real consumer project usually means `npm link`, `pnpm link`, or `file:` references. All of these pollute your lockfile, break with pnpm's content-addressable store, or behave differently from a real install.

Smuggle takes a different approach: it packs your local package the same way `npm publish` would, then directly replaces the installed copy in `node_modules`. Your lockfile and `package.json` stay untouched. When you're done, originals are restored automatically.

## Install

### npm

```sh
npm install -g smuggle-cli
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

### 1. Publish local packages

In your package directory (or workspace root):

```sh
smuggle publish
```

This packs the package and registers it locally in `~/.smuggle/packages/`. In a pnpm workspace, you'll be prompted to select which packages to publish.

Use `--all` to skip the prompt and publish all non-private packages:

```sh
smuggle publish --all
```

### 2. Install into a consumer project

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

### 3. List registered packages

```sh
smuggle list
```

### 4. Remove a registered package

```sh
smuggle unpublish @scope/my-pkg
```

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
- **No `package.json` changes** — version ranges are preserved
- **Automatic cleanup** — originals are restored on exit, even on ctrl-c
- **Hash-based change detection** — only triggers cache busting and Vite restarts when the packed output actually changes
- **Workspace support** — detects pnpm workspaces and scans all member packages for matching dependencies

## Supported package managers

- pnpm (including workspaces)
- npm
- yarn

## License

MIT
