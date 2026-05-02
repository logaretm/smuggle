# Getting Started

## Installation

Pick your favorite package manager.

<InstallTabs />

## Quick start

<Steps>

<li>

### Register your local package

In your package directory (or workspace root):

```sh
smuggle publish
```

This packs the package and registers it locally in `~/.smuggle/packages/`. In a pnpm workspace, you'll be prompted to select which packages to publish.

Use `--all` to skip the prompt and publish all non-private packages:

```sh
smuggle publish --all
```

</li>

<li>

### Swap into your consumer project

In your consumer project directory:

```sh
smuggle
```

This finds registered packages that match the consumer's dependencies, lets you select which ones to swap, backs up the originals, extracts your local packages into `node_modules`, and watches for changes.

Press `ctrl-c` to stop watching and restore the originals.

</li>

<li>

### Iterate

Edit your source package — smuggle detects the change, re-packs, and re-extracts automatically. Bundler caches (Vite, Next.js, webpack) are cleared and `vite.config.*` is touched to trigger a restart.

</li>

</Steps>

## Supported package managers

- pnpm (including workspaces)
- npm
- yarn
