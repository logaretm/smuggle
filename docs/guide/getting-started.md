# Getting Started

## Installation

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

## Quick start

### 1. Register your local package

In your package directory (or workspace root):

```sh
smuggle publish
```

This packs the package and registers it locally in `~/.smuggle/packages/`. In a pnpm workspace, you'll be prompted to select which packages to publish.

Use `--all` to skip the prompt and publish all non-private packages:

```sh
smuggle publish --all
```

### 2. Swap into your consumer project

In your consumer project directory:

```sh
smuggle
```

This will find registered packages that match the consumer's dependencies, let you select which ones to swap, back up the originals, extract your local packages into `node_modules`, and watch for changes.

Press `ctrl-c` to stop watching and restore the originals.

### 3. Iterate

Edit your source package — smuggle detects the change, re-packs, and re-extracts automatically. Bundler caches (Vite, Next.js, webpack) are cleared and `vite.config.*` is touched to trigger a restart.

## Supported package managers

- pnpm (including workspaces)
- npm
- yarn
