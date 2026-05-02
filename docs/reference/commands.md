# Commands

<CommandsOverview />

## `smuggle publish`

Pack and register a local package for later use.

```sh
smuggle publish              # single package
smuggle publish --all        # all workspace packages
smuggle publish --path ./pkg # specific directory
```

In a pnpm workspace, you'll be prompted to select which packages to publish unless `--all` is passed.

## `smuggle install`

Swap registered packages into a consumer project's `node_modules`. Also available as bare `smuggle`.

```sh
smuggle                          # interactive, with file watcher
smuggle install --all --once     # non-interactive, no watcher
smuggle install --ci             # CI mode (--all --once + NDJSON output)
```

What it does:

1. Finds registered packages that match the consumer's dependencies
2. Lets you select which ones to swap (unless `--all`)
3. Auto-includes transitive dependencies that are also registered
4. Backs up the originals from `node_modules`
5. Extracts your local packages into `node_modules`
6. Clears bundler caches and touches `vite.config.*` to trigger a restart
7. Watches for changes and re-swaps on change (unless `--once`)
8. Restores originals on exit

## `smuggle add`

Add unreleased packages to your `package.json` dependencies and swap them in.

```sh
smuggle add @scope/my-pkg
smuggle add @scope/pkg-a @scope/pkg-b    # multiple packages
smuggle add @scope/my-pkg --dev          # as devDependency
smuggle add @scope/my-pkg --once         # don't start file watcher
```

<Callout kind="tip" title="When to use add">

`smuggle add` is for packages that haven't been released yet and aren't in your `package.json`. If the package is already a dependency, just run `smuggle` with no subcommand.

</Callout>

## `smuggle dev`

Swap local packages and run your dev server in one command.

```sh
smuggle dev                        # auto-detect dev script from package.json
smuggle dev -- npm run start       # custom command
smuggle dev --restart              # restart server on change instead of HMR
smuggle dev --all                  # skip package selection prompt
```

Combines `smuggle install` with your dev server — swaps packages, starts the server, and watches for changes.

## `smuggle list`

List all registered local packages.

```sh
smuggle list
smuggle list --ci    # JSON output
```

## `smuggle unpublish`

Remove a registered package from the local store.

```sh
smuggle unpublish @scope/my-pkg    # by name
smuggle unpublish                  # interactive selection
smuggle unpublish --all            # remove all
```
