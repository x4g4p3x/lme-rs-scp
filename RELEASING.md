# Releasing

Publish dependencies first: lme-rs 0.2.7, then lme-rs-agent 0.2.1, then this adapter.
Core 0.2.0 introduced captured-data fields on public CachedFit; 0.2.1 retains them.

1. Remove sibling paths/patches and resolve the registry lockfile.
2. Run `task preflight`, the official SCP client test, and `cargo publish --dry-run --locked`.
3. Commit reviewed changes, push, and inspect Windows/Linux CI.
4. Publish with `cargo publish --locked`; tag the published commit and create its release.
5. Install the registry package into an isolated directory and rerun stdio/client tests
   against that executable. Verify GitHub heads/tags and crates.io versions.

Keep tokens out of source and logs. Do not overwrite a published version.
