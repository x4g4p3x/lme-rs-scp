# Agent and contributor pre-flights

Thin Rust Science Context Protocol adapter around the sibling `lme-rs-agent` crate.
Statistical logic belongs in `lme-rs`; shared orchestration belongs in `lme-rs-agent`.

Before handing off Rust changes, run:

```powershell
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

Keep the SCP surface workflow-oriented. Do not mirror every MCP tool by default.
