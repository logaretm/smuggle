# Why Smuggle?

Testing local packages in a real consumer project usually means `npm link`, `pnpm link`, `yalc`, or `file:` references. Each one leaves a trail.

<BeforeAfter />

> Smuggle packs your local package the same way `npm publish` would, then directly replaces the installed copy in `node_modules`. Your lockfile and `package.json` stay untouched.

## The trade-offs of the alternatives

- **`npm link` / `pnpm link`** — creates symlinks that confuse bundlers, break with pnpm's content-addressable store, and behave differently from a real install.
- **`file:` references** — pollute your lockfile and `package.json` with local paths that break for other contributors.
- **`yalc`** — closer, but still modifies your lockfile and requires manual cleanup.

## Design principles

<Principles />
