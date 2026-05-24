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
smuggle @scope/my-pkg            # install a specific package (even if not yet in package.json)
smuggle @scope/pkg-a @scope/pkg-b --dev  # install as devDependencies
```

What it does:

1. If `node_modules` is missing, runs your package manager's install first
2. Finds registered packages that match the consumer's dependencies (or uses the names you passed)
3. If any named packages aren't in `package.json`, temporarily injects `file:` references, runs `pm install` to resolve transitive deps, then reverts `package.json` and lockfile on exit
4. Lets you select which ones to swap (unless `--all`)
5. Auto-includes transitive dependencies that are also registered
6. Backs up the originals from `node_modules`
7. Extracts your local packages into `node_modules`
8. Clears bundler caches and touches `vite.config.*` to trigger a restart
9. Watches for changes and re-swaps on change (unless `--once`)
10. Restores originals on exit

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
