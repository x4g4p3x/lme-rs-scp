use anyhow::{Context, Result};
use lme_rs_agent::ModelKind;
use lme_rs_scp::{ExperimentContextInput, LmeScpServer, ScpFitModelRequest};

fn main() -> Result<()> {
    let data_path = std::env::args()
        .nth(1)
        .context("usage: cargo run --example fit_model -- <sleepstudy.csv>")?;
    let result = LmeScpServer::new().fit_model(ScpFitModelRequest {
        context: ExperimentContextInput {
            experiment_id: Some("sleepstudy-demo".to_string()),
            run_id: Some("fit-1".to_string()),
            objective: Some("Estimate the population Days slope".to_string()),
        },
        model_kind: ModelKind::Lmm,
        formula: "Reaction ~ Days + (1 | Subject)".to_string(),
        data_path,
        reml: Some(true),
        family: None,
        link: None,
        n_agq: None,
        start: None,
    })?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
