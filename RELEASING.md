# Releasing

Publish dependencies first: lme-rs 0.2.6, then lme-rs-agent 0.2.0, then this adapter.
The minor core version accounts for the new captured-data fields on public CachedFit.

1. Remove sibling paths/patches and resolve the registry lockfile.
2. Run `task preflight`, the official SCP client test, and `cargo publish --dry-run --locked`.
3. Commit reviewed changes, push, and inspect Windows/Linux CI.
4. Publish with `cargo publish --locked`; tag the published commit and create its release.
5. Install the registry package into an isolated directory and rerun stdio/client tests
   against that executable. Verify GitHub heads/tags and crates.io versions.

Keep tokens out of source and logs. Do not overwrite a published version.
