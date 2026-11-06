# Changesets

This folder is managed by [Changesets](https://github.com/changesets/changesets).

ODK ships one version across every artifact it publishes: the npm SDK
(`@omnidotdev/odk`), the Rust crates (`odk-schema`, `odk-cli`, `odk-pdk`), and
the Python PDK (`odk-pdk`). Changesets is the single source of truth for that
version.

To record a change for the next release, run:

```sh
bun changeset
```

Add the changeset against `@omnidotdev/odk`. On merge to `master`, the release
workflow opens a "version packages" PR that bumps `@omnidotdev/odk` and
propagates the same version to the Rust crates and the Python PDK (see
`scripts/sync-versions.ts`). Merging that PR publishes every artifact.

Do not hand-edit package versions or run `npm publish` / `cargo publish` /
`twine upload` by hand; the release workflow owns publishing.
