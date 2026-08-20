---
"smuggle-cli": minor
---

Adds `smuggle ui`, a terminal interface for driving a session.

It opens with nothing hijacked and lists everything you have published. Press space on a package to start serving your local build of it, and space again to stop; smuggle re-pins your lockfile and reinstalls each time, so you can swap a dependency in and out without restarting anything. Quitting ends the session and puts the published packages back.

Alongside the session view there is a store view for evicting or repacking what you have published, and a doctor view that checks your certificate authority, the background daemon, the `/etc/hosts` redirect, and which registry this project actually resolves through. The doctor's worst result is shown in the header from every view, so a stale daemon or a registry smuggle is not intercepting is visible immediately rather than looking like nothing happening.
