# Contributing

The Rust adapter owns scientific context, resource lifecycle, provenance, and transport.
Statistical implementations stay in lme-rs; shared orchestration stays in lme-rs-agent.
Release builds use registry dependencies. During co-development, a temporary sibling
path may be used with the declared version; remove it and regenerate the registry lockfile
before release.

Run `task ci` (format, clippy, check, test) and `task preflight` (also cargo audit).
The snapshot regression in the shared core checks source replacement/removal, and the
adapter tests cover lineage, export, experiment isolation, cancellation, and cleanup.

## Official client test

The server requires no Python. This optional compatibility check uses Python 3.11 and
unmodified official client source pinned to `cea5398564032aea65a78e246d06c30ae945e03f`.
Only dependencies imported by its client/server initialization are installed; hub/cloud
dependencies are unnecessary for this stdio test.

```powershell
git clone https://github.com/InternScience/scp.git .compat/scp-upstream
git -C .compat/scp-upstream checkout --detach cea5398564032aea65a78e246d06c30ae945e03f
python -m venv .venv
.venv/Scripts/python -m pip install -r tests/requirements-scp-client.txt
.venv/Scripts/python -m pip install --no-deps .compat/scp-upstream
cargo test --locked
.venv/Scripts/python tests/scp_client.py --source .compat/scp-upstream --binary target/debug/lme-rs-scp.exe
```

On Unix use `.venv/bin/python` and omit the binary's `.exe` suffix. CI performs this
check on both operating systems. For an installed executable, set `LME_SCP_TEST_BINARY`
for the Rust stdio test and pass its path to `tests/scp_client.py`.

The pinned official client's JSON-RPC error path references an undefined `McpError`
instead of its imported `ScpError`. Known workflow failures return standard `isError`
tool results and are covered by the test. Unknown protocol methods/resources remain
JSON-RPC errors; consumers of this upstream revision may encounter that client defect.
