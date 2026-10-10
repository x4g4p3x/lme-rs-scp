# Agent and contributor pre-flights

Thin Rust Science Context Protocol adapter around published `lme-rs-agent`.
Statistical logic belongs in `lme-rs`; shared orchestration belongs in `lme-rs-agent`.

Before handing off Rust changes, run:

```powershell
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo audit
```

Keep the SCP surface workflow-oriented. Do not mirror every MCP tool by default.

Release dependencies must resolve from crates.io without sibling paths. See
CONTRIBUTING.md for the pinned official-client test, and RELEASING.md for delivery.
