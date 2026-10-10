//! SCP scientific workflows around the shared statistical core.
use crate::provenance::dataset_provenance;
use crate::*;
use chrono::Utc;
use lme_rs_agent::*;
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router, ErrorData, Json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Debug, thiserror::Error)]
pub enum ScpAdapterError {
    #[error(transparent)]
    Agent(#[from] AgentApiError),
    #[error("failed to serialize scientific result: {0}")]
    Serialization(#[from] serde_json::Error),
}
fn invalid(message: impl Into<String>) -> ScpAdapterError {
    AgentApiError::InvalidInput(message.into()).into()
}
pub(crate) fn to_protocol_error(error: ScpAdapterError) -> ErrorData {
    match error {
        ScpAdapterError::Agent(AgentApiError::InvalidInput(m) | AgentApiError::NotFound(m)) => {
            ErrorData::invalid_params(m, None)
        }
        other => ErrorData::internal_error(other.to_string(), None),
    }
}
struct Record {
    resource: ScientificResource,
    _reservation: OwnedSemaphorePermit,
    fit_id: Option<String>,
    dataset: Option<DatasetSnapshot>,
}
struct FitGuard {
    api: LmeAgentApi,
    id: Option<String>,
}
impl Drop for FitGuard {
    fn drop(&mut self) {
        if let Some(id) = &self.id {
            let _ = self.api.forget_model(id);
        }
    }
}

#[cfg(test)]
type TestGate = Arc<Mutex<Option<(Arc<std::sync::Barrier>, Arc<std::sync::Barrier>)>>>;

#[derive(Clone)]
pub struct LmeScpServer {
    pub(crate) api: LmeAgentApi,
    pub(crate) tool_router: rmcp::handler::server::router::tool::ToolRouter<Self>,
    records: Arc<Mutex<BTreeMap<String, Record>>>,
    capacity: Arc<Semaphore>,
    pub(crate) jobs: Arc<Semaphore>,
    #[cfg(test)]
    pub(crate) test_gate: TestGate,
}
impl Default for LmeScpServer {
    fn default() -> Self {
        Self::new()
    }
}
impl LmeScpServer {
    pub fn new() -> Self {
        Self {
            api: LmeAgentApi::new(),
            tool_router: Self::tool_router(),
            records: Arc::new(Mutex::new(BTreeMap::new())),
            capacity: Arc::new(Semaphore::new(64)),
            jobs: Arc::new(Semaphore::new(2)),
            #[cfg(test)]
            test_gate: Arc::new(Mutex::new(None)),
        }
    }
    pub fn api(&self) -> &LmeAgentApi {
        &self.api
    }
    fn reserve(&self) -> Result<OwnedSemaphorePermit, ScpAdapterError> {
        self.capacity.clone().try_acquire_owned().map_err(|_| {
            invalid("resource capacity reached (64); forget resources before retrying")
        })
    }
    fn insert(&self, record: Record) -> Result<(), ScpAdapterError> {
        let mut records = self.records.lock().expect("resource lock poisoned");
        for parent in &record.resource.parents {
            let existing = records
                .get(parent)
                .ok_or_else(|| invalid("input resource was forgotten during computation"))?;
            if existing.resource.context.experiment_id != record.resource.context.experiment_id {
                return Err(invalid(
                    "input resources must belong to the same experiment",
                ));
            }
        }
        records.insert(record.resource.uri.clone(), record);
        Ok(())
    }
    fn fit_record(
        &self,
        id: &str,
    ) -> Result<(ScientificResource, DatasetSnapshot), ScpAdapterError> {
        let records = self.records.lock().expect("resource lock poisoned");
        let record = records
            .get(&format!("lme-rs://fits/{id}"))
            .ok_or_else(|| invalid("unknown fitted-model resource"))?;
        Ok((
            record.resource.clone(),
            record.dataset.clone().expect("fit has captured dataset"),
        ))
    }
    pub fn resources(&self) -> Vec<ScientificResource> {
        self.records
            .lock()
            .expect("resource lock poisoned")
            .values()
            .map(|r| r.resource.clone())
            .collect()
    }
    pub fn read_resource(&self, uri: &str) -> Result<ScientificResource, ScpAdapterError> {
        self.records
            .lock()
            .expect("resource lock poisoned")
            .get(uri)
            .map(|r| r.resource.clone())
            .ok_or_else(|| invalid("unknown scientific resource"))
    }
    /// Forget a resource and its descendants, releasing cached fits and reservations.
    pub fn forget_resource(&self, uri: &str) -> Result<ForgottenResources, ScpAdapterError> {
        let mut records = self.records.lock().expect("resource lock poisoned");
        if !records.contains_key(uri) {
            return Err(invalid("unknown scientific resource"));
        }
        let mut removed = BTreeSet::from([uri.to_owned()]);
        loop {
            let before = removed.len();
            for record in records.values() {
                if record.resource.parents.iter().any(|p| removed.contains(p)) {
                    removed.insert(record.resource.uri.clone());
                }
            }
            if removed.len() == before {
                break;
            }
        }
        for id in &removed {
            if let Some(record) = records.remove(id) {
                if let Some(fit_id) = record.fit_id {
                    let _ = self.api.forget_model(&fit_id);
                }
            }
        }
        Ok(ForgottenResources {
            uris: removed.into_iter().collect(),
        })
    }
    pub fn fit_model(
        &self,
        request: ScpFitModelRequest,
    ) -> Result<ScpFitModelResult, ScpAdapterError> {
        let reservation = self.reserve()?;
        let started_at = Utc::now();
        let (context, agent_request) = request.split();
        if context.experiment_id.len() > 4096
            || context.run_id.as_ref().is_some_and(|s| s.len() > 4096)
            || context.objective.as_ref().is_some_and(|s| s.len() > 16384)
        {
            return Err(invalid("experiment context exceeds size limits"));
        }
        let summary = self.api.fit_model(agent_request.clone())?;
        let mut guard = FitGuard {
            api: self.api.clone(),
            id: Some(summary.fit_id.clone()),
        };
        let dataset = self.api.dataset_snapshot(&summary.fit_id)?;
        let provenance =
            ModelProvenance::capture(&agent_request, &summary, started_at, Utc::now(), &dataset);
        let result = ScpFitModelResult {
            context: context.clone(),
            resource: FittedModelResource::from(summary),
            provenance,
        };
        self.insert(Record {
            resource: ScientificResource {
                uri: result.resource.uri.clone(),
                name: "fit_model".into(),
                media_type: result.resource.media_type.clone(),
                context,
                parents: vec![],
                document: serde_json::to_value(&result)?,
            },
            _reservation: reservation,
            fit_id: guard.id.clone(),
            dataset: Some(dataset),
        })?;
        guard.id = None;
        Ok(result)
    }
    pub fn assess_model(
        &self,
        request: AssessModelRequest,
    ) -> Result<AnalysisResult<AssessmentSummary>, ScpAdapterError> {
        let reservation = self.reserve()?;
        let started_at = Utc::now();
        let parameters = serde_json::to_value(&request)?;
        let (source, dataset) = self.fit_record(&request.fit_id)?;
        let mut parents = vec![source.uri.clone()];
        if let Some(other) = &request.comparison_fit_id {
            if other == &request.fit_id {
                return Err(invalid("comparison requires two distinct fitted models"));
            }
            let (other_source, other_dataset) = self.fit_record(other)?;
            if source.context.experiment_id != other_source.context.experiment_id
                || dataset.bytes() != other_dataset.bytes()
            {
                return Err(invalid(
                    "comparison models must share experiment and captured input bytes",
                ));
            }
            parents.push(other_source.uri);
        }
        let diagnostics = self.api.diagnostics(&request.fit_id)?;
        let confidence_intervals = self.api.confidence_intervals(ConfidenceIntervalsRequest {
            fit_id: request.fit_id.clone(),
            level: request.confidence.level,
            method: request.confidence.method,
            parameters: request.confidence.parameters,
        })?;
        let cross_validation = request
            .cross_validation
            .map(|o| {
                self.api.cross_validate(CrossValidateRequest {
                    fit_id: request.fit_id.clone(),
                    group_col: o.group_col,
                    n_splits: o.n_splits,
                    seed: o.seed,
                    n_jobs: o.n_jobs,
                })
            })
            .transpose()?;
        let comparison = request
            .comparison_fit_id
            .map(|other| {
                self.api.compare_models(CompareModelsRequest {
                    fit_id_a: request.fit_id.clone(),
                    fit_id_b: other,
                })
            })
            .transpose()?;
        self.analysis(
            "assess_model",
            source.context,
            parents,
            vec![dataset_provenance(&dataset)],
            parameters,
            started_at,
            AssessmentSummary {
                diagnostics,
                confidence_intervals,
                cross_validation,
                comparison,
            },
            reservation,
        )
    }
    pub fn infer_marginals(
        &self,
        request: InferMarginalsRequest,
    ) -> Result<AnalysisResult<MarginalInferenceSummary>, ScpAdapterError> {
        let reservation = self.reserve()?;
        let started_at = Utc::now();
        let parameters = serde_json::to_value(&request)?;
        let (source, dataset) = self.fit_record(&request.grid.fit_id)?;
        let means = self.api.marginal_means(request.grid.clone())?;
        let comparisons = if request.comparisons {
            Some(self.api.pairwise_comparisons(PairwiseComparisonsRequest {
                grid: request.grid,
                adjust: request.adjust,
            })?)
        } else {
            None
        };
        self.analysis(
            "infer_marginals",
            source.context,
            vec![source.uri],
            vec![dataset_provenance(&dataset)],
            parameters,
            started_at,
            MarginalInferenceSummary { means, comparisons },
            reservation,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn analysis<T: serde::Serialize>(
        &self,
        operation: &str,
        context: ExperimentContext,
        parents: Vec<String>,
        datasets: Vec<DatasetProvenance>,
        parameters: serde_json::Value,
        started_at: chrono::DateTime<Utc>,
        summary: T,
        reservation: OwnedSemaphorePermit,
    ) -> Result<AnalysisResult<T>, ScpAdapterError> {
        let result = AnalysisResult {
            context: context.clone(),
            resource: AnalysisResource {
                uri: format!("lme-rs://results/{}", uuid::Uuid::new_v4()),
                media_type: "application/vnd.lme-rs.analysis+json".into(),
                summary,
            },
            provenance: ActivityProvenance::new(
                operation,
                started_at,
                parents.clone(),
                datasets,
                parameters,
            ),
        };
        self.insert(Record {
            resource: ScientificResource {
                uri: result.resource.uri.clone(),
                name: operation.into(),
                media_type: result.resource.media_type.clone(),
                context,
                parents,
                document: serde_json::to_value(&result)?,
            },
            _reservation: reservation,
            fit_id: None,
            dataset: None,
        })?;
        Ok(result)
    }
    pub fn export_experiment(
        &self,
        request: ExportExperimentRequest,
    ) -> Result<ExperimentExport, ScpAdapterError> {
        let (resources, snapshots): (Vec<_>, Vec<_>) = {
            let records = self.records.lock().expect("resource lock poisoned");
            let selected: Vec<_> = records
                .values()
                .filter(|r| r.resource.context.experiment_id == request.experiment_id)
                .collect();
            if selected.is_empty() {
                return Err(invalid("unknown experiment"));
            }
            (
                selected.iter().map(|r| r.resource.clone()).collect(),
                selected.iter().filter_map(|r| r.dataset.clone()).collect(),
            )
        };
        let mut datasets = BTreeMap::new();
        let mut export_bytes = 0usize;
        for snapshot in snapshots {
            let provenance = dataset_provenance(&snapshot);
            if datasets.contains_key(&provenance.sha256) {
                continue;
            }
            let csv = if request.include_data {
                export_bytes += snapshot.bytes().len();
                if export_bytes > 32 * 1024 * 1024 {
                    return Err(invalid(
                        "embedded export data exceeds 32 MiB; export metadata only",
                    ));
                }
                Some(
                    std::str::from_utf8(snapshot.bytes())
                        .map_err(|_| invalid("captured CSV is not UTF-8"))?
                        .to_owned(),
                )
            } else {
                None
            };
            datasets.insert(provenance.sha256.clone(), ExportDataset { provenance, csv });
        }
        Ok(ExperimentExport {
            schema_version: "lme-rs.scp.export/v1".into(),
            experiment_id: request.experiment_id,
            exported_at: Utc::now(),
            resources,
            datasets: datasets.into_values().collect(),
        })
    }
    async fn compute<T: Send + 'static>(
        &self,
        operation: impl FnOnce(Self) -> Result<T, ScpAdapterError> + Send + 'static,
    ) -> Result<Json<T>, ErrorData> {
        let permit = self.jobs.clone().try_acquire_owned().map_err(|_| {
            ErrorData::invalid_params(
                "server busy: two workflows are running; retry after completion",
                None,
            )
        })?;
        let server = self.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            #[cfg(test)]
            {
                let gate = server.test_gate.lock().unwrap().take();
                if let Some((started, release)) = gate {
                    started.wait();
                    release.wait();
                }
            }
            operation(server).map(Json).map_err(to_protocol_error)
        })
        .await
        .map_err(|_| ErrorData::internal_error("scientific worker failed", None))?
    }
}

#[tool_router]
impl LmeScpServer {
    #[tool(
        name = "fit_model",
        description = "Fit an LM/LMM/GLMM/NLMM with captured input bytes, experiment context, and a retrievable scientific resource."
    )]
    async fn fit_model_tool(
        &self,
        Parameters(request): Parameters<ScpFitModelRequest>,
    ) -> Result<Json<ScpFitModelResult>, ErrorData> {
        self.compute(move |s| s.fit_model(request)).await
    }
    #[tool(
        name = "assess_model",
        description = "Assess a model with diagnostics and confidence intervals, optionally grouped CV and a same-experiment mixed-model comparison."
    )]
    async fn assess_model_tool(
        &self,
        Parameters(request): Parameters<AssessModelRequest>,
    ) -> Result<Json<AnalysisResult<AssessmentSummary>>, ErrorData> {
        self.compute(move |s| s.assess_model(request)).await
    }
    #[tool(
        name = "infer_marginals",
        description = "Compute LM/LMM marginal means and optional adjusted comparisons, preserving experiment context and input lineage."
    )]
    async fn infer_marginals_tool(
        &self,
        Parameters(request): Parameters<InferMarginalsRequest>,
    ) -> Result<Json<AnalysisResult<MarginalInferenceSummary>>, ErrorData> {
        self.compute(move |s| s.infer_marginals(request)).await
    }
    #[tool(
        name = "export_experiment",
        description = "Export linked scientific resources and dataset digests as JSON, optionally including exact captured CSV text (32 MiB total)."
    )]
    async fn export_experiment_tool(
        &self,
        Parameters(request): Parameters<ExportExperimentRequest>,
    ) -> Result<Json<ExperimentExport>, ErrorData> {
        self.compute(move |s| s.export_experiment(request)).await
    }
    #[tool(
        name = "forget_resource",
        description = "Forget a resource and all derived descendants; release its fit, captured data, and resource capacity."
    )]
    async fn forget_resource_tool(
        &self,
        Parameters(request): Parameters<ForgetResourceRequest>,
    ) -> Result<Json<ForgottenResources>, ErrorData> {
        self.compute(move |s| s.forget_resource(&request.uri)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resource_capacity_rejects_work_and_forgetting_releases_it() {
        let server = LmeScpServer::new();
        let request = || {
            serde_json::from_value(serde_json::json!({"model_kind":"lm","formula":"Reaction ~ Days","data_path":std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/sleepstudy.csv")})).unwrap()
        };
        let result = server.fit_model(request()).unwrap();
        let budget = server.capacity.clone().try_acquire_many_owned(63).unwrap();
        assert!(server
            .fit_model(request())
            .unwrap_err()
            .to_string()
            .contains("capacity"));
        server.forget_resource(&result.resource.uri).unwrap();
        assert!(server.fit_model(request()).is_ok());
        drop(budget);
    }
}
