---
"smuggle-cli": minor
---

Smuggle now works by intercepting registry traffic instead of writing into `node_modules`.

Run `smuggle setup` once. It installs a local certificate authority and a background daemon, and asks for your password a single time. From then on, `smuggle` in a project hijacks the packages you select: your package manager resolves them normally and receives your local build instead of the published one.

This fixes the cases the old approach could not survive. Reinstalling no longer wipes your local copy, a warm cache no longer serves the published package behind your back, and bundlers no longer load a stale prebundle, because the change is one your package manager can actually see.

Interception lasts exactly as long as the command runs. Quitting restores your lockfile and reinstalls the published packages, and a session that is killed outright is cleaned up the next time you run smuggle.

`smuggle install`, `smuggle dev` and `--ci` are gone. Bare `smuggle` (or `smuggle hijack`) replaces install; smuggle no longer installs anything itself, so run your own package manager while it is up. Lockfile pinning supports npm and pnpm; yarn and bun report that they are unsupported rather than appearing to work.
