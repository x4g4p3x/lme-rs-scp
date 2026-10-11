use lme_rs_scp::*;
use serde_json::json;
use sha2::{Digest, Sha256};

fn fit(
    server: &LmeScpServer,
    path: &std::path::Path,
    formula: &str,
    experiment: &str,
) -> ScpFitModelResult {
    server.fit_model(serde_json::from_value(json!({"model_kind":"lmm","formula":formula,"data_path":path,"reml":false,"context":{"experiment_id":experiment,"run_id":"run:1","objective":"Assess treatment"}})).unwrap()).unwrap()
}
#[test]
fn complete_workflow_uses_captured_data_and_exports_linked_results() {
    let root = std::env::temp_dir().join(format!("lme-scp-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("input.csv");
    let bytes = include_bytes!("data/marginal.csv");
    std::fs::write(&path, bytes).unwrap();
    let server = LmeScpServer::new();
    let reduced = fit(
        &server,
        &path,
        "y ~ A + B + x + (1 | subject)",
        "experiment:1",
    );
    let full = fit(
        &server,
        &path,
        "y ~ A * B + x + (1 | subject)",
        "experiment:1",
    );
    assert_eq!(
        full.provenance.dataset.sha256,
        format!("{:x}", Sha256::digest(bytes))
    );
    assert_eq!(full.provenance.dataset.size_bytes, bytes.len() as u64);
    assert_eq!(full.provenance.agent.version, "0.2.1");
    std::fs::remove_file(&path).unwrap();
    let assessed = server.assess_model(serde_json::from_value(json!({"fit_id":full.resource.summary.fit_id,"comparison_fit_id":reduced.resource.summary.fit_id,"cross_validation":{"group_col":"subject","n_splits":2,"seed":42}})).unwrap()).unwrap();
    assert_eq!(assessed.context, full.context);
    assert_eq!(assessed.provenance.input_resources.len(), 2);
    assert!(assessed
        .resource
        .summary
        .cross_validation
        .unwrap()
        .rmse
        .is_finite());
    assert!(assessed
        .resource
        .summary
        .comparison
        .unwrap()
        .p_value
        .is_finite());
    let inferred = server.infer_marginals(serde_json::from_value(json!({"fit_id":full.resource.summary.fit_id,"terms":["A"],"by":["B"],"at":{"x":0.25},"adjust":"holm"})).unwrap()).unwrap();
    assert_eq!(inferred.context, full.context);
    assert_eq!(inferred.resource.summary.means.rows.len(), 6);
    assert_eq!(inferred.resource.summary.comparisons.unwrap().rows.len(), 6);
    let read = server.read_resource(&inferred.resource.uri).unwrap();
    assert_eq!(read.parents, vec![full.resource.uri.clone()]);
    let exported = server
        .export_experiment(ExportExperimentRequest {
            experiment_id: "experiment:1".into(),
            include_data: true,
        })
        .unwrap();
    assert_eq!(exported.resources.len(), 4);
    assert_eq!(exported.datasets.len(), 1);
    assert_eq!(exported.datasets[0].csv.as_ref().unwrap().as_bytes(), bytes);
    let metadata = server
        .export_experiment(ExportExperimentRequest {
            experiment_id: "experiment:1".into(),
            include_data: false,
        })
        .unwrap();
    assert!(serde_json::to_value(metadata).unwrap()["datasets"][0]
        .get("csv")
        .is_none());
    assert!(server
        .export_experiment(ExportExperimentRequest {
            experiment_id: "missing".into(),
            include_data: false
        })
        .is_err());
    let forgotten = server.forget_resource(&full.resource.uri).unwrap();
    assert_eq!(forgotten.uris.len(), 3);
    assert!(server.read_resource(&inferred.resource.uri).is_err());
    assert_eq!(server.api().list_models().fits.len(), 1);
    std::fs::remove_dir(&root).unwrap();
}

#[test]
fn cross_experiment_comparison_is_rejected_without_publishing_a_result() {
    let server = LmeScpServer::new();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/marginal.csv");
    let a = fit(
        &server,
        &path,
        "y ~ A + B + x + (1 | subject)",
        "experiment:a",
    );
    let b = fit(
        &server,
        &path,
        "y ~ A * B + x + (1 | subject)",
        "experiment:b",
    );
    let request = serde_json::from_value(
        json!({"fit_id":a.resource.summary.fit_id,"comparison_fit_id":b.resource.summary.fit_id}),
    )
    .unwrap();
    assert!(server
        .assess_model(request)
        .unwrap_err()
        .to_string()
        .contains("experiment"));
    assert_eq!(server.resources().len(), 2);
}
