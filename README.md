# lme-rs-scp

Rust [Science Context Protocol](https://github.com/InternScience/scp) adapter for
[lme-rs](https://github.com/x4g4p3x/lme-rs), built on the published
[lme-rs-agent](https://github.com/x4g4p3x/lme-rs-agent) shared core.

The adapter exposes scientific workflows over SCP's MCP-compatible stdio contract:

```text
fit_model -> assess_model -> infer_marginals -> export_experiment
     |            |               |
     +------------+---------------+-> retrievable resources + linked provenance
```

Statistical algorithms stay in lme-rs. This release uses lme-rs-agent 0.2.1 and the
published lme-rs 0.2.7 engine. The executable requires no Python. The compatibility
test uses the official Python SCP client at a pinned revision.

## Install and run

```powershell
cargo install lme-rs-scp --version 0.2.1 --locked
$env:LME_DATA_ROOT = "C:\path\to\csv-data"
lme-rs-scp
```

The process speaks newline-delimited JSON-RPC on stdin/stdout. Configure an SCP/MCP
stdio client to spawn it. Relative CSV paths resolve under LME_DATA_ROOT; canonical
paths outside that root are rejected. LME_MCP_DATA_ROOT remains a compatibility alias.

Source builds use registry dependencies and require no sibling checkout:

```powershell
git clone https://github.com/x4g4p3x/lme-rs-scp.git
cd lme-rs-scp
cargo build --release --locked
```

## Workflow

### Fit a model

The existing fit_model request/result shape is preserved, with additional shared-core
version provenance. LM/LMM/GLMM/built-in NLMM use the same model-family options as the
shared agent API.

```json
{
  "context": {"experiment_id":"experiment:study", "run_id":"run:1", "objective":"Estimate treatment effects"},
  "model_kind":"lmm",
  "formula":"y ~ treatment + x + (1 | subject)",
  "data_path":"study.csv",
  "reml":false
}
```

The result contains context, a model resource with its FitSummary, and provenance:
adapter/core/engine versions, exact captured CSV digest and length, effective model
configuration, convergence, and activity timestamps. Missing experiment IDs become
UUID URNs. Input bytes and the parsed frame are retained at fitting; later operations
use that frame even if the source path changes or disappears.

### Assess the model

```json
{
  "fit_id":"<fit_id>",
  "confidence":{"method":"wald", "level":0.95},
  "cross_validation":{"group_col":"subject", "n_splits":5, "seed":42},
  "comparison_fit_id":"<optional second fit_id>"
}
```

Diagnostics and confidence intervals are always included. Confidence defaults to
95% Wald intervals; optional parameters select fixed-effect names. Profile intervals
support LMM/GLMM. Optional grouped CV supports LMM/GLMM. Optional likelihood-ratio
comparison requires compatible mixed models fit with ML, from the same experiment
and captured dataset. Both model resource URIs are recorded as inputs. Inspect
convergence and CV all_converged fields when interpreting results.

### Infer marginal effects

```json
{
  "fit_id":"<fit_id>",
  "terms":["treatment"],
  "by":["sex"],
  "at":{"age":40.0},
  "weights":"equal",
  "ddf_method":"auto",
  "level":0.95,
  "comparisons":true,
  "adjust":"holm"
}
```

LM/LMM only. Target and conditioning factors must be distinct fitted categorical
factors; multiple terms define interaction cells. Omitted numeric references use
sample means. Equal/proportional nuisance weights and asymptotic/residual (LM)/
Satterthwaite/Kenward-Roger (LMM) inference follow the shared core. Auto selects
residual df for LM and Satterthwaite for LMM. Holm is the default pairwise adjustment;
Bonferroni, Tukey, and none are available. Comparisons can be omitted. Adjustment
families remain separate within each conditioning stratum. Mean intervals are pointwise;
pairwise output contains raw/adjusted p-values rather than simultaneous intervals.
GLMM/NLMM marginal grids and grids with offsets are unsupported by this engine.

### Retrieve and export

resources/list lists fitted-model and analysis resources. resources/read retrieves
the complete original envelope by its lme-rs URI. Analysis resources inherit their
model's context and identify input resources, dataset digests, parameters, versions,
and activity times. Resource documents are immutable within the session.

```json
{"experiment_id":"experiment:study", "include_data":true}
```

export_experiment returns an lme-rs.scp.export/v1 JSON bundle containing linked
resources and deduplicated dataset digests. Exact captured UTF-8 CSV text is included
only when requested; otherwise the export contains metadata/results. Save the returned
bundle through the client. It preserves the scientific record across server restarts;
it does not serialize native fitted objects or restore live fit handles. Embedded data
is limited to 32 MiB total.

forget_resource accepts a resource URI and removes it and all derived descendants.
Forgetting a fitted model releases its core fit, captured data, and catalog capacity.
Running analyses cannot publish results after their input resource is forgotten.

## Execution and limits

Wire workflows run on blocking workers outside the transport runtime. At most two
workflows run concurrently; excess calls return an isError busy result for retry.
Up to 64 scientific resources are retained. CSV inputs are limited to 256 MiB.
The core limits CV to 2–100 folds and 1–4 explicit engine workers (default one),
AGQ to 1–25, and marginal grids to 128 target/conditioning cells (4096 full grid rows).
Experiment/run IDs are limited to 4096 bytes and objectives to 16384 bytes.

Standard notifications/cancelled discards an undelivered workflow result. A running
numerical computation drains while holding its worker slot; newly created fits/results
are cleaned afterward. Previously delivered input models remain available. Cancelling
an explicit forget_resource operation may still complete its requested deletion.
Resources, snapshots, and fit handles are process-local and clear on restart. There
is no automatic persistence, hub registration, network transport, or authentication
in this local stdio release.

Known workflow failures use standard isError tool results. Unknown protocol methods
and resources use JSON-RPC errors. The pinned official client's RPC-error exception
path has an upstream naming defect; see CONTRIBUTING.md for the tested boundary.

## Development and validation

Run task ci and task preflight. Rust tests cover the scientific workflow, exports,
experiment isolation, cancellation, and lifecycle. The shared core compares marginal
inference with independent R emmeans/lmerTest references. Windows/Linux CI also drives
the real executable through unmodified official SCP client source pinned to
cea5398564032aea65a78e246d06c30ae945e03f.

See [CONTRIBUTING.md](CONTRIBUTING.md), [RELEASING.md](RELEASING.md), and
[CHANGELOG.md](CHANGELOG.md). The direct library example remains available:

```powershell
cargo run --release --example fit_model -- C:\path\to\sleepstudy.csv
```

## License

MIT.
