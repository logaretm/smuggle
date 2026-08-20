# Changesets

Smuggle has one version number wearing several hats: the `smuggle` crate, the
four `@smuggle-cli/*` binary packages, and the `smuggle-cli` package that
depends on them. Changesets tracks `npm/smuggle-cli` and `scripts/sync-versions.ts`
copies whatever it decides onto everything else, so `changeset version` is the
only thing that ever picks a number.

## Adding a changeset

Run this in any PR that changes something a smuggle user would notice:

```sh
pnpm changeset
```

Pick `patch` / `minor` / `major`, describe the change from the user's point of
view, and commit the generated file in `.changeset/`. What you write lands in
`npm/smuggle-cli/CHANGELOG.md` and in the GitHub release, so write it for
someone reading release notes, not for a reviewer reading the diff.

For a change that ships nothing user-visible (a refactor, a test, docs, CI),
record that explicitly instead:

```sh
pnpm changeset --empty
```

An empty changeset satisfies the PR check and is consumed by the next release
without bumping the version.

## Releasing

Merging a changeset to `main` does not release. It opens (or updates) a
`chore: release` pull request holding the version bump and the CHANGELOG entry
every pending changeset adds up to. That PR sits there collecting further
merges, so a run of five PRs becomes one release rather than five.

Releasing is merging that PR. CI then tags `vX.Y.Z`, builds the four platform
binaries, publishes them plus `smuggle-cli` to npm, and cuts a GitHub release
from the new CHANGELOG section.

The one manual step left is the Homebrew tap:

```sh
gh workflow run update-formula.yml -R logaretm/homebrew-tap \
  -f formula=smuggle -f version=X.Y.Z -f repo=logaretm/smuggle
```
