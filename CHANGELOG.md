# Changelog

## 0.2.0 - 2026-10-10

- Use published lme-rs-agent 0.2.0 and pinned lme-rs 0.2.6; upgrade rmcp to 3.5.1.
- Capture provenance from the exact bytes used to fit and analyze a model.
- Implement resource listing/retrieval, context inheritance, parent lineage, and cascading cleanup.
- Add model assessment (diagnostics, confidence intervals, optional grouped CV and comparison),
  marginal inference, and experiment export with optional captured CSV data.
- Bound wire workflows to two numerical workers and session resources to 64.
- Handle request cancellation while workers drain, cleaning undelivered results and fits.
- Test real stdio and official SCP client workflows at an immutable upstream revision.
- Add Windows/Linux CI and registry package verification.

## 0.1.0

- Initial one-fit SCP harness with experiment context and dataset provenance.
