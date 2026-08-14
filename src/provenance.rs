//! Provenance records for SCP scientific results.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use chrono::{DateTime, Utc};
use lme_rs_agent::{FitModelRequest, FitSummary, ModelKind, ENGINE_NAME, ENGINE_VERSION};
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
#[derive(Debug, Clone, Serialize, JsonSchema)]
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
    ) -> io::Result<Self> {
        let dataset = hash_dataset(Path::new(&summary.data_path))?;
        Ok(Self {
            schema_version: "lme-rs.scp.provenance/v1".to_string(),
            adapter: SoftwareProvenance {
                name: env!("CARGO_PKG_NAME").to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            engine: SoftwareProvenance {
                name: ENGINE_NAME.to_string(),
                version: ENGINE_VERSION.to_string(),
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
        })
    }
}

fn hash_dataset(path: &Path) -> io::Result<DatasetProvenance> {
    let mut file = File::open(path)?;
    let size_bytes = file.metadata()?.len();
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }

    Ok(DatasetProvenance {
        path: path.display().to_string(),
        media_type: "text/csv".to_string(),
        sha256: format!("{:x}", digest.finalize()),
        size_bytes,
    })
}
