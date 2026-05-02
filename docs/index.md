---
layout: home
hero:
  name: Smuggle
  text: Local npm packages, no hacks needed
  tagline: No symlinks, no lockfile pollution, no registry overrides. Just real files in node_modules.
  actions:
    - theme: brand
      text: Get Started
      link: /guide/getting-started
    - theme: alt
      text: Why Smuggle?
      link: /guide/why
features:
  - title: No symlinks
    details: Packages are real files in node_modules, just like a normal install.
  - title: No lockfile changes
    details: Your pnpm-lock.yaml, package-lock.json, or yarn.lock stays untouched.
  - title: Automatic cleanup
    details: Originals are restored on exit, even on ctrl-c.
  - title: Instant feedback
    details: File watcher re-packs and re-extracts on every change with hash-based change detection.
---
