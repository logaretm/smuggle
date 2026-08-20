# Getting Started

## Requirements

macOS. Smuggle uses launchd, the login keychain and `/etc/hosts`, so Linux and Windows are not supported yet.

Lockfile pinning supports npm and pnpm. Yarn and bun are detected and reported rather than silently doing nothing.

## Installation

Pick your favorite package manager.

<InstallTabs />

## Quick start

<Steps>

<li>

### Set up your machine

Once, ever:

```sh
smuggle setup
```

This generates a local certificate authority, trusts it, and installs a background daemon. It asks for your password a single time, and nothing after that will ask again.

Nothing is intercepted by this step. The daemon stays inert until a session registers with it.

</li>

<li>

### Register your local package

In your package directory, or a workspace root:

```sh
smuggle publish
```

This packs the package the same way `npm publish` would and stores the tarball in `~/.smuggle/packages/`. In a workspace you will be asked which packages to publish.

Use `--all` to skip the prompt and publish every non-private package:

```sh
smuggle publish --all
```

</li>

<li>

### Serve it to your project

In the consumer project:

```sh
smuggle
```

Smuggle matches your registered packages against the project's dependencies and asks which to serve. It then pins those entries in your lockfile to your local build and runs your package manager once, so the swap takes effect immediately.

Press `ctrl-c` to stop. Your lockfile is restored and the published packages are reinstalled.

</li>

<li>

### Iterate

Edit your package and smuggle repacks it automatically. Run your package manager again and the new build arrives, because your package manager is genuinely fetching it rather than reading files someone put there.

</li>

</Steps>

## The terminal UI

```sh
smuggle ui
```

Opens with nothing served, and lists everything you have published. Press `space` on a package to start serving it and `space` again to stop, without restarting anything.

There is also a store view for evicting and repacking published packages, and a doctor view that checks your certificate authority, the background daemon, the `/etc/hosts` redirect, and which registry this project actually resolves through. That last check is worth knowing about: if your project is configured to use a company registry rather than the public one, the doctor view is where you will see it.

## Undoing everything

```sh
smuggle cleanup
```

Removes the certificate authority, the shell profile entry, the background daemon, and any redirect a crashed session left behind. Safe to run at any time, including when nothing is installed.
