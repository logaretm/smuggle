# Commands

<CommandsOverview />

## `smuggle setup`

Install the local certificate authority and the background daemon. Run once per machine.

```sh
smuggle setup
```

Generates a CA in `~/.smuggle/ca/`, trusts it in your login keychain, points `NODE_EXTRA_CA_CERTS` at it in your shell profile, and installs a launchd daemon. It asks for your password a single time; sessions never ask again.

Nothing is intercepted by this command. The daemon stays inert until a session registers with it.

Re-run it after upgrading smuggle. The daemon runs a copy of the binary staged in a root-owned directory, so a new version is not picked up until you restage it. Sessions will tell you when this is needed rather than misbehaving.

## `smuggle publish`

Pack and register a local package.

```sh
smuggle publish              # single package
smuggle publish --all        # all workspace packages
smuggle publish --path ./pkg # specific directory
```

Packs the package the same way `npm publish` would and stores the tarball in `~/.smuggle/packages/`. In a workspace you will be prompted to select packages unless `--all` is passed.

## `smuggle hijack`

Serve registered packages to a project until interrupted. Also available as bare `smuggle`.

```sh
smuggle                             # interactive selection
smuggle --all                       # serve every matching package
smuggle @scope/pkg-a @scope/pkg-b   # serve specific packages
smuggle -v                          # log every request, not just hijacked ones
```

What it does:

1. Detects the project's lockfile and the registries it resolves through
2. Matches registered packages against the project's dependencies, or uses the names you passed
3. Pulls in any registered package your selection depends on
4. Registers with the daemon, which points those registries at a local proxy
5. Rewrites the integrity of those packages in your lockfile to your local build
6. Runs your package manager once so the change takes effect
7. Repacks automatically when you edit your package
8. On exit, restores the lockfile, stops intercepting, and reinstalls the published packages

Smuggle does not install anything itself beyond that first run. While the session is up, run your package manager as often as you like and your local build is what arrives.

## `smuggle ui`

The same session, in a terminal UI.

```sh
smuggle ui
```

Opens with nothing served. `space` starts and stops serving the selected package, re-pinning and reinstalling each time. `tab` cycles between the session, store and doctor views. `q` quits, ending the session exactly as `ctrl-c` does.

Needs an interactive terminal. Use bare `smuggle` when piping output or running non-interactively.

## `smuggle list`

List all registered local packages.

```sh
smuggle list
```

## `smuggle unpublish`

Remove a registered package from the local store.

```sh
smuggle unpublish @scope/my-pkg    # by name
smuggle unpublish                  # interactive selection
smuggle unpublish --all            # remove all
```

## `smuggle cleanup`

Remove everything `smuggle setup` installed, plus anything a crashed session left behind.

```sh
smuggle cleanup
```

Removes the certificate authority, the keychain entry, the shell profile line, the background daemon, and any leftover `/etc/hosts` redirect. Safe to run at any time.

## `smuggle proxy`

Run the interception proxy in the foreground. Sessions start this through the daemon, so you rarely need it, but it is useful for watching traffic directly.

```sh
sudo smuggle proxy --verbose
```

Needs root, because it binds port 443 and edits `/etc/hosts`.
