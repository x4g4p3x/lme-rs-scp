# lme-rs-scp

Thin [Science Context Protocol (SCP)](https://github.com/InternScience/scp) server adapter
for [lme-rs](https://github.com/x4g4p3x/lme-rs) mixed-effects models.

This initial harness exposes one representative scientific workflow:

```text
SCP fit_model request
        |
        v
   LmeAgentApi             shared validation and session semantics
        |
        v
      lme-rs               statistical engine
        |
        v
fitted-model resource + experiment context + provenance
```

It deliberately does not duplicate the full MCP tool catalog. The first boundary to prove
is how a fitted model and its source data become a reusable, traceable scientific result.

## Architecture

```text
                    lme-rs
                       ^
                       |
                lme-rs-agent
               /            \
              ^              ^
       lme-rs-mcp       lme-rs-scp
```

The official SCP Python SDK currently implements edge servers with the MCP-compatible
JSON-RPC tool/resource contract and supports stdio and SSE transports. This Rust harness
uses that compatible tool contract over stdio. Hub registration, authentication, SSE, and
persistent external storage are intentionally later integration steps.

## Current workflow

The only tool is `fit_model`. It accepts:

- optional `context` (`experiment_id`, `run_id`, `objective`);
- `model_kind` (`lm`, `lmm`, `glmm`, or `nlmm`);
- formula and server-local CSV `data_path`;
- model-family-specific REML, family/link, AGQ, and start options.

The result contains:

- resolved experiment context;
- an `lme-rs://fits/<fit_id>` model resource and structured `FitSummary`;
- adapter and engine versions;
- the canonical dataset path, byte length, media type, and SHA-256 digest;
- effective model configuration, convergence, observation count, and activity timestamps.

The shared agent core currently pins `lme-rs 0.2.2`; that exact engine version is recorded
in every provenance envelope.

Model handles are process-local in this harness. A persistent SCP resource store is not yet
implemented.

## Run

Until `lme-rs-agent 0.1.0` is published, source builds require sibling checkouts:

```text
parent/
├─ lme-rs-agent/
└─ lme-rs-scp/
```

```powershell
cargo build --release --locked
$env:LME_DATA_ROOT = "C:\path\to\csv-data"
target\release\lme-rs-scp.exe
```

The process speaks newline-delimited JSON-RPC over stdin/stdout. `LME_DATA_ROOT` is
recommended: relative paths resolve below it and canonicalized paths outside it are rejected.

Run the direct envelope example with:

```powershell
cargo run --release --example fit_model -- C:\path\to\sleepstudy.csv
```

## Development

```powershell
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

## Scope after the harness

Next operations should be shaped as scientific workflows with the same context/provenance
contract: prediction, model comparison, confidence intervals, bootstrap, and grouped
cross-validation. They should continue to call `lme-rs-agent`, never reimplement statistics.

## License

MIT.
