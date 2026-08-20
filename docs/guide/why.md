# Why Smuggle?

Testing local packages in a real consumer project usually means `npm link`, `pnpm link`, `yalc`, or `file:` references. Each one leaves a trail.

<BeforeAfter />

> Smuggle answers your package manager's registry requests with your local build. Your package manager resolves, downloads and installs exactly as it always does, so what lands in `node_modules` is a real install rather than an imitation of one.

## The trade-offs of the alternatives

- **`npm link` / `pnpm link`** creates symlinks that confuse bundlers, break with pnpm's content-addressable store, and behave differently from a real install.
- **`file:` references** pollute your lockfile and `package.json` with local paths that break for other contributors.
- **`yalc`** is closer, but still modifies your lockfile and requires manual cleanup.

## Why not just copy files into node_modules?

Smuggle used to do exactly that, and it is worth explaining why it stopped.

Writing files into `node_modules` works right up until anything else touches it. Reinstalling wipes your build. A warm cache serves the published package instead. Bundlers keep a prebundle keyed on the lockfile, which has not changed, so they serve stale code. Each of those is fixable with another workaround, and the workarounds multiply.

The underlying reason is that package managers are content addressed. They decide what to install from a hash in your lockfile, and they will happily serve that content from a cache without going near the network. Files that appear underneath them are invisible to all of it.

Intercepting the registry inverts the problem. Instead of working around the package manager, smuggle gives it something it can see: a different hash, which no cache can satisfy, and a fetch it has to make. Everything downstream then behaves normally, because as far as your tooling is concerned, nothing unusual happened.

## What smuggle touches

Being honest about this matters more than a short list of things it avoids.

While a session runs, smuggle changes two things on your machine: the registry hostnames in `/etc/hosts` point at a local proxy, and the integrity hashes for the packages you are serving are rewritten in your lockfile. Both are undone when the session ends, and a session that is killed outright is cleaned up the next time you run smuggle.

Your `package.json` is never touched, no symlinks are created, and nothing is written into `node_modules` by smuggle itself. Your package manager puts it there.

## Design principles

<Principles />
