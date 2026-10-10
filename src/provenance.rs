//! Provenance records for SCP scientific results.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use lme_rs_agent::{
    DatasetSnapshot, FitModelRequest, FitSummary, ModelKind, ENGINE_NAME, ENGINE_VERSION,
};
use schemars::JsonSchema;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Software identity recorded for the adapter and statistical engine.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SoftwareProvenance {
    pub name: String,
    pub version: String,
}

/// Content-addressed source dataset used by the model fit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct DatasetProvenance {
    pub path: String,
    pub media_type: String,
    pub sha256: String,
    pub size_bytes: u64,
}

/// Effective model configuration submitted to the neutral agent core.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FitConfiguration {
    pub model_kind: ModelKind,
    pub formula: String,
    pub reml: Option<bool>,
    pub family: Option<String>,
    pub link: Option<String>,
    pub n_agq: Option<usize>,
    pub start: Option<BTreeMap<String, f64>>,
}

/// Observable result state relevant to reproducibility and downstream decisions.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FitOutcome {
    pub fit_id: String,
    pub converged: bool,
    pub num_obs: usize,
}

/// Timing for the fit activity represented by this result.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FitActivity {
    pub operation: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

/// Versioned provenance envelope attached to an SCP fitted-model resource.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ModelProvenance {
    pub schema_version: String,
    pub adapter: SoftwareProvenance,
    pub agent: SoftwareProvenance,
    pub engine: SoftwareProvenance,
    pub dataset: DatasetProvenance,
    pub configuration: FitConfiguration,
    pub outcome: FitOutcome,
    pub activity: FitActivity,
}

impl ModelProvenance {
    pub(crate) fn capture(
        request: &FitModelRequest,
        summary: &FitSummary,
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
        snapshot: &DatasetSnapshot,
    ) -> Self {
        let dataset = dataset_provenance(snapshot);
        Self {
            schema_version: "lme-rs.scp.provenance/v1".to_string(),
            adapter: SoftwareProvenance {
                name: env!("CARGO_PKG_NAME").to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            engine: SoftwareProvenance {
                name: ENGINE_NAME.to_string(),
                version: ENGINE_VERSION.to_string(),
            },
            agent: SoftwareProvenance {
                name: "lme-rs-agent".into(),
                version: lme_rs_agent::AGENT_VERSION.into(),
            },
            dataset,
            configuration: FitConfiguration {
                model_kind: request.model_kind,
                formula: request.formula.clone(),
                reml: summary.reml,
                family: summary.family.clone(),
                link: summary.link.clone(),
                n_agq: summary.n_agq,
                start: summary.start.clone(),
            },
            outcome: FitOutcome {
                fit_id: summary.fit_id.clone(),
                converged: summary.converged,
                num_obs: summary.num_obs,
            },
            activity: FitActivity {
                operation: "fit_model".to_string(),
                started_at,
                completed_at,
            },
        }
    }
}

pub(crate) fn dataset_provenance(snapshot: &DatasetSnapshot) -> DatasetProvenance {
    DatasetProvenance {
        path: snapshot.path.display().to_string(),
        media_type: "text/csv".to_string(),
        sha256: format!("{:x}", Sha256::digest(snapshot.bytes())),
        size_bytes: snapshot.bytes().len() as u64,
    }
}
