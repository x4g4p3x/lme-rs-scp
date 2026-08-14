//! SCP server adapter around the shared statistical agent core.

use chrono::Utc;
use lme_rs_agent::{AgentApiError, LmeAgentApi};
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router, ErrorData, Json};

use crate::mapping::{FittedModelResource, ScpFitModelRequest, ScpFitModelResult};
use crate::provenance::ModelProvenance;

#[derive(Debug, thiserror::Error)]
pub enum ScpAdapterError {
    #[error(transparent)]
    Agent(#[from] AgentApiError),
    #[error("failed to capture input provenance: {0}")]
    Provenance(#[from] std::io::Error),
}

#[derive(Clone)]
pub struct LmeScpServer {
    api: LmeAgentApi,
}

impl LmeScpServer {
    pub fn new() -> Self {
        Self {
            api: LmeAgentApi::new(),
        }
    }

    pub fn api(&self) -> &LmeAgentApi {
        &self.api
    }

    /// Execute the representative SCP fit workflow and attach scientific provenance.
    pub fn fit_model(
        &self,
        request: ScpFitModelRequest,
    ) -> Result<ScpFitModelResult, ScpAdapterError> {
        let started_at = Utc::now();
        let (context, agent_request) = request.split();
        let summary = self.api.fit_model(agent_request.clone())?;
        let completed_at = Utc::now();
        let provenance =
            match ModelProvenance::capture(&agent_request, &summary, started_at, completed_at) {
                Ok(provenance) => provenance,
                Err(error) => {
                    let _ = self.api.forget_model(&summary.fit_id);
                    return Err(ScpAdapterError::Provenance(error));
                }
            };

        Ok(ScpFitModelResult {
            context,
            resource: FittedModelResource::from(summary),
            provenance,
        })
    }
}

impl Default for LmeScpServer {
    fn default() -> Self {
        Self::new()
    }
}

fn to_protocol_error(error: ScpAdapterError) -> ErrorData {
    match error {
        ScpAdapterError::Agent(AgentApiError::InvalidInput(message))
        | ScpAdapterError::Agent(AgentApiError::NotFound(message)) => {
            ErrorData::invalid_params(message, None)
        }
        ScpAdapterError::Agent(AgentApiError::Computation(message)) => {
            ErrorData::internal_error(message, None)
        }
        ScpAdapterError::Provenance(error) => {
            ErrorData::internal_error(format!("failed to capture input provenance: {error}"), None)
        }
    }
}

#[tool_router(server_handler)]
impl LmeScpServer {
    #[tool(
        name = "fit_model",
        description = "Fit one LM, LMM, GLMM, or built-in NLMM from a CSV dataset and return an SCP scientific resource with experiment context and reproducibility provenance."
    )]
    fn fit_model_tool(
        &self,
        Parameters(request): Parameters<ScpFitModelRequest>,
    ) -> Result<Json<ScpFitModelResult>, ErrorData> {
        self.fit_model(request).map(Json).map_err(to_protocol_error)
    }
}
