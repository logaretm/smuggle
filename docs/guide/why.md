# Why Smuggle?

Testing local packages in a real consumer project usually means `npm link`, `pnpm link`, or `file:` references. All of these have problems:

- **`npm link` / `pnpm link`** — creates symlinks that confuse bundlers, break with pnpm's content-addressable store, and behave differently from a real install.
- **`file:` references** — pollute your lockfile and `package.json` with local paths that break for other contributors.
- **`yalc`** — closer, but still modifies your lockfile and requires manual cleanup.

## A different approach

Smuggle packs your local package the same way `npm publish` would, then directly replaces the installed copy in `node_modules`. Your lockfile and `package.json` stay untouched.

```
smuggle publish                     smuggle install
  │                                   │
  ▼                                   ▼
  Pack files (like npm publish)       Find matches in consumer deps
  │                                   │
  ▼                                   ▼
  Store tarball in ~/.smuggle/        Backup node_modules originals
                                      │
                                      ▼
                                      Extract tarball into node_modules
                                      │
                                      ▼
                                      Watch source dirs for changes
                                      │
                                      ▼
                                      On change: re-pack → hash check
                                      → extract if changed → bust caches
                                      │
                                      ▼
                                      On exit: restore originals
```

## Design decisions

- **No symlinks** — packages are real files in `node_modules`, just like a normal install
- **No lockfile changes** — your `pnpm-lock.yaml` / `package-lock.json` / `yarn.lock` stays untouched
- **No `.npmrc` changes** — no registry overrides needed
- **No `package.json` changes** — version ranges are preserved (`smuggle add` is the exception — it adds the dependency)
- **Automatic cleanup** — originals are restored on exit, even on ctrl-c
- **Hash-based change detection** — only triggers cache busting and Vite restarts when the packed output actually changes
- **Workspace support** — detects pnpm workspaces and scans all member packages for matching dependencies
