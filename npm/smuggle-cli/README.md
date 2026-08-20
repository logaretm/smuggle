# smuggle-cli

Test your local npm packages in a real project, without npm link.

[Documentation](https://awad.dev/smuggle)

## Why?

Testing a local package in a real consumer project usually means `npm link`, `pnpm link`, `yalc`, or a `file:` reference. Symlinks confuse bundlers and break pnpm's content-addressable store, and `file:` references leave local paths in your lockfile and `package.json`.

Smuggle takes a different route: it intercepts your package manager's registry requests and answers them with your local build. Your package manager resolves, downloads and installs exactly as it normally would. It just receives your code instead of the published tarball.

That means a real install, not an imitation of one. Reinstalling does not wipe your changes, a warm cache does not quietly serve the published version, and bundlers see a genuine dependency change rather than files that appeared underneath them.

## Requirements

macOS. Smuggle uses launchd, the login keychain and `/etc/hosts`, so Linux and Windows are not supported yet.

Lockfile pinning supports npm and pnpm. Yarn and bun are detected and reported rather than silently doing nothing.

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

## Setup

Once per machine:

```sh
smuggle setup
```

This generates a local certificate authority, trusts it, and installs a background daemon. It asks for your password a single time. Nothing is intercepted by this step: the daemon stays inert until you start a session.

To undo all of it, including anything a crashed session left behind:

```sh
smuggle cleanup
```

## Usage

| Command | Description |
|---------|-------------|
| `smuggle setup` | Install the local CA and background daemon (one time) |
| `smuggle publish` | Pack and register a local package |
| `smuggle` or `smuggle hijack` | Serve registered packages to this project until ctrl-c |
| `smuggle ui` | The same session, in a terminal UI |
| `smuggle list` | List registered packages |
| `smuggle unpublish <pkg>` | Remove a registered package |
| `smuggle cleanup` | Remove the CA, the daemon, and any leftover redirect |

### Register a package

In your package directory, or a workspace root:

```sh
smuggle publish
```

This packs the package the same way `npm publish` would and stores the tarball in `~/.smuggle/packages/`. In a workspace you will be asked which packages to publish; `--all` skips the prompt.

### Serve it to a project

In the consumer project:

```sh
smuggle
```

Smuggle matches your registered packages against the project's dependencies, asks which to serve, and then holds them for as long as it runs. It pins those entries in your lockfile to your local build and runs your package manager once, so the change takes effect immediately.

Press `ctrl-c` to stop. Your lockfile is restored and the published packages are reinstalled.

While a session is running you can install as often as you like. Your local build is what arrives every time, because your package manager is genuinely fetching it.

### Iterate

Edit your package and smuggle repacks it. Run your package manager again, or use `smuggle ui` and press `space` to reinstall, and the new build lands in `node_modules`.

### Terminal UI

```sh
smuggle ui
```

Opens with nothing served. Pick packages from the list with `space` to start and stop serving them without restarting anything. Alongside the session view there is a store view for evicting and repacking what you have published, and a doctor view that checks your CA, the daemon, the redirect and which registry this project actually resolves through.

## How it works

`smuggle setup` installs a certificate authority and a root daemon. The daemon does nothing until a session registers with it.

Starting a session points the registry hostnames at a local address through `/etc/hosts` and stands up a proxy holding a certificate your machine trusts. Requests for packages you are serving are answered from `~/.smuggle`; everything else is forwarded upstream untouched.

Package managers are content addressed, so smuggle also rewrites the integrity hash of those packages in your lockfile. That is what makes a warm cache miss: a hash nothing has ever seen cannot be served from a cache, so the fetch has to go out, and it reaches the proxy. This is why you never need to clear a cache by hand.

The redirect exists only while a session runs. Ending one removes it, restores the lockfile and reinstalls the published packages. A session killed outright is cleaned up the next time you run smuggle.
