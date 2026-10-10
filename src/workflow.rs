//! Workflow envelopes; numerical implementations stay in the shared core.
use crate::{DatasetProvenance, ExperimentContext, FitActivity, SoftwareProvenance};
use chrono::{DateTime, Utc};
use lme_rs_agent::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConfidenceOptions {
    #[serde(default = "confidence_level")]
    pub level: f64,
    #[serde(default = "wald")]
    pub method: ConfidenceIntervalMethod,
    #[serde(default)]
    pub parameters: Option<Vec<String>>,
}
fn confidence_level() -> f64 {
    0.95
}
fn wald() -> ConfidenceIntervalMethod {
    ConfidenceIntervalMethod::Wald
}
impl Default for ConfidenceOptions {
    fn default() -> Self {
        Self {
            level: 0.95,
            method: wald(),
            parameters: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CrossValidationOptions {
    pub group_col: String,
    #[serde(default = "folds")]
    pub n_splits: usize,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub n_jobs: Option<usize>,
}
fn folds() -> usize {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AssessModelRequest {
    pub fit_id: String,
    #[serde(default)]
    pub confidence: ConfidenceOptions,
    #[serde(default)]
    pub cross_validation: Option<CrossValidationOptions>,
    /// Optional second mixed model from the same experiment/dataset; both must use ML.
    #[serde(default)]
    pub comparison_fit_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AssessmentSummary {
    pub diagnostics: DiagnosticsSummary,
    pub confidence_intervals: ConfidenceIntervalsSummary,
    pub cross_validation: Option<CrossValidationSummary>,
    pub comparison: Option<CompareModelsSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InferMarginalsRequest {
    #[serde(flatten)]
    pub grid: MarginalMeansRequest,
    #[serde(default = "yes")]
    pub comparisons: bool,
    #[serde(default)]
    pub adjust: PairwiseAdjustment,
}
fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct MarginalInferenceSummary {
    pub means: MarginalMeansSummary,
    pub comparisons: Option<PairwiseComparisonsSummary>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AnalysisResource<T> {
    pub uri: String,
    pub media_type: String,
    pub summary: T,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ActivityProvenance {
    pub schema_version: String,
    pub adapter: SoftwareProvenance,
    pub agent: SoftwareProvenance,
    pub engine: SoftwareProvenance,
    pub activity: FitActivity,
    pub input_resources: Vec<String>,
    pub datasets: Vec<DatasetProvenance>,
    pub parameters: Value,
}
impl ActivityProvenance {
    pub(crate) fn new(
        operation: &str,
        started_at: DateTime<Utc>,
        input_resources: Vec<String>,
        datasets: Vec<DatasetProvenance>,
        parameters: Value,
    ) -> Self {
        Self {
            schema_version: "lme-rs.scp.activity/v1".into(),
            adapter: SoftwareProvenance {
                name: env!("CARGO_PKG_NAME").into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            agent: SoftwareProvenance {
                name: "lme-rs-agent".into(),
                version: AGENT_VERSION.into(),
            },
            engine: SoftwareProvenance {
                name: ENGINE_NAME.into(),
                version: ENGINE_VERSION.into(),
            },
            activity: FitActivity {
                operation: operation.into(),
                started_at,
                completed_at: Utc::now(),
            },
            input_resources,
            datasets,
            parameters,
        }
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AnalysisResult<T> {
    pub context: ExperimentContext,
    pub resource: AnalysisResource<T>,
    pub provenance: ActivityProvenance,
}

/// Immutable record exposed by resources/read and experiment exports.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ScientificResource {
    pub uri: String,
    pub name: String,
    pub media_type: String,
    pub context: ExperimentContext,
    pub parents: Vec<String>,
    pub document: Value,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ExportExperimentRequest {
    pub experiment_id: String,
    /// Include exact captured CSV text. Default exports metadata/results only.
    #[serde(default)]
    pub include_data: bool,
}
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ExportDataset {
    pub provenance: DatasetProvenance,
    /// CSV text is omitted unless explicitly requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub csv: Option<String>,
}
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ExperimentExport {
    pub schema_version: String,
    pub experiment_id: String,
    pub exported_at: DateTime<Utc>,
    pub resources: Vec<ScientificResource>,
    pub datasets: Vec<ExportDataset>,
}
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ForgetResourceRequest {
    pub uri: String,
}
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ForgottenResources {
    pub uris: Vec<String>,
}
