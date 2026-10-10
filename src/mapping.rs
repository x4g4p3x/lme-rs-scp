//! Mapping between SCP experiment envelopes and protocol-neutral agent DTOs.

use std::collections::BTreeMap;

use lme_rs_agent::{FitModelRequest, FitSummary, ModelKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::provenance::ModelProvenance;

/// Optional scientific context supplied by an SCP workflow.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ExperimentContextInput {
    /// Stable experiment identifier supplied by the caller. A UUID URN is generated when omitted.
    #[serde(default)]
    pub experiment_id: Option<String>,
    /// Optional workflow/run identifier within the experiment.
    #[serde(default)]
    pub run_id: Option<String>,
    /// Optional human-readable scientific objective.
    #[serde(default)]
    pub objective: Option<String>,
}

/// Resolved context attached to every scientific result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ExperimentContext {
    pub experiment_id: String,
    pub run_id: Option<String>,
    pub objective: Option<String>,
}

impl ExperimentContextInput {
    pub(crate) fn resolve(self) -> ExperimentContext {
        ExperimentContext {
            experiment_id: self
                .experiment_id
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| format!("urn:uuid:{}", Uuid::new_v4())),
            run_id: self.run_id.filter(|value| !value.trim().is_empty()),
            objective: self.objective.filter(|value| !value.trim().is_empty()),
        }
    }
}

/// One SCP scientific workflow entry point: fit a model from a server-local CSV dataset.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScpFitModelRequest {
    /// Persistent experiment context for lifecycle and provenance correlation.
    #[serde(default)]
    pub context: ExperimentContextInput,
    /// Statistical model family: `lm`, `lmm`, `glmm`, or `nlmm`.
    pub model_kind: ModelKind,
    /// Model formula. LM/LMM/GLMM use Wilkinson syntax; NLMM uses three-part syntax.
    pub formula: String,
    /// Absolute or data-root-relative CSV path on the SCP server host.
    pub data_path: String,
    /// REML setting for LMM/NLMM.
    #[serde(default)]
    pub reml: Option<bool>,
    /// GLMM distribution family.
    #[serde(default)]
    pub family: Option<String>,
    /// Optional GLMM link function.
    #[serde(default)]
    pub link: Option<String>,
    /// Adaptive Gauss-Hermite quadrature points for GLMM/NLMM.
    #[serde(default)]
    pub n_agq: Option<usize>,
    /// Optional named NLMM starting values.
    #[serde(default)]
    pub start: Option<BTreeMap<String, f64>>,
}

impl ScpFitModelRequest {
    pub(crate) fn split(self) -> (ExperimentContext, FitModelRequest) {
        let context = self.context.resolve();
        let request = FitModelRequest {
            model_kind: self.model_kind,
            formula: self.formula,
            data_path: self.data_path,
            reml: self.reml,
            family: self.family,
            link: self.link,
            n_agq: self.n_agq,
            start: self.start,
        };
        (context, request)
    }
}

/// SCP-addressable representation of the fitted model held by this process.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FittedModelResource {
    pub uri: String,
    pub media_type: String,
    pub summary: FitSummary,
}

impl From<FitSummary> for FittedModelResource {
    fn from(summary: FitSummary) -> Self {
        Self {
            uri: format!("lme-rs://fits/{}", summary.fit_id),
            media_type: "application/vnd.lme-rs.fit+json".to_string(),
            summary,
        }
    }
}

/// Structured scientific result returned by the SCP `fit_model` workflow.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ScpFitModelResult {
    pub context: ExperimentContext,
    pub resource: FittedModelResource,
    pub provenance: ModelProvenance,
}
