---
name: smuggle
description: Use when the user wants to test a local npm package in a consumer project, serve a local build in place of a published one, or avoid npm link / symlink issues. Also use when the user mentions "smuggle", "local package testing", or asks how to test a package they're developing locally.
allowed-tools: Bash, Read, Glob, Grep
---

# Smuggle: local npm package testing

Smuggle intercepts your package manager's registry requests and answers them with a local build. The package manager resolves, downloads and installs exactly as it normally would, so what lands in `node_modules` is a real install rather than files copied over the top.

macOS only. Lockfile pinning supports npm and pnpm; yarn and bun are reported as unsupported rather than silently doing nothing.

## Prerequisites

Check that smuggle is installed:

```sh
smuggle --help
```

If not, install it with one of:

```sh
npm install -g smuggle-cli
brew install logaretm/tap/smuggle
cargo install smuggle
```

Then, once per machine:

```sh
smuggle setup
```

This installs a local certificate authority and a background daemon, asking for a password once. Nothing is intercepted by it; the daemon stays inert until a session starts. Re-run it after upgrading smuggle, since the daemon runs a staged copy of the binary. A session will say so if it is out of date.

## Commands

### `smuggle publish`

Run inside the package being developed, or a workspace root. Packs it the same way `npm publish` would and stores the tarball in `~/.smuggle/packages/`.

```sh
smuggle publish              # interactive selection in a workspace
smuggle publish --all        # every non-private package
smuggle publish --path ./pkg # a specific directory
```

### `smuggle` (or `smuggle hijack`)

Run in the consumer project. Blocks until ctrl-c.

```sh
smuggle                            # pick from registered packages matching the project
smuggle --all                      # serve every match
smuggle @scope/pkg-a @scope/pkg-b  # serve specific packages
smuggle -v                         # log every request, not just hijacked ones
```

It pins the integrity of those packages in the lockfile to the local build, runs the package manager once, and repacks whenever the source changes. On exit it restores the lockfile, stops intercepting, and reinstalls the published packages.

While it runs, the user can install as often as they like and the local build is what arrives.

### `smuggle ui`

The same session as a terminal UI. Opens with nothing served; `space` toggles a package, `tab` switches between session, store and doctor views, `q` quits. Needs an interactive terminal.

### Other commands

```sh
smuggle list                     # registered packages
smuggle unpublish @scope/pkg     # remove one from the store
smuggle cleanup                  # remove the CA, daemon, and any leftover redirect
```

## Typical flow

1. `smuggle setup` if it has never been run on this machine.
2. `smuggle publish` in the package directory.
3. `smuggle` in the consumer project, and leave it running.
4. Edit the package. Smuggle repacks; run the package manager again to pick it up, or press `space` twice in `smuggle ui`.
5. ctrl-c when done.

## Troubleshooting

**Nothing appears in the log during an install.** The project may resolve through a registry smuggle is not intercepting, which is common with company registries. `smuggle ui` has a doctor view showing the registry npm actually reports for the project. `--registry <url>` adds one explicitly.

**A session says the daemon is out of date.** Run `smuggle setup` again. The daemon runs a copy of the binary staged in a root-owned directory, so upgrading smuggle does not update it.

**Installs fail after a crash.** Run `smuggle cleanup`, which removes any leftover `/etc/hosts` redirect. A lockfile left pinned by a killed session is repaired the next time smuggle runs.

**Do not hand-edit versions.** Smuggle serves whatever `smuggle publish` packed, at whatever version the source declares.
