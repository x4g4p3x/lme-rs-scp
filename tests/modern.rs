use rmcp::{
    model::*, serve_client_with_lifecycle, transport::TokioChildProcess, ClientLifecycleMode,
};
use serde_json::json;
#[tokio::test]
async fn modern_discovery_serves_resource_documents_and_workflow_errors() {
    tokio::time::timeout(std::time::Duration::from_secs(30),async {
        let binary = std::env::var_os("LME_SCP_TEST_BINARY").unwrap_or_else(||env!("CARGO_BIN_EXE_lme-rs-scp").into());
        let mut command = tokio::process::Command::new(binary);
        command.env("LME_DATA_ROOT",std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data"));
        command.env_remove("LME_MCP_DATA_ROOT");
        #[cfg(windows)]
        command.creation_flags(0x08000000);
        let client = serve_client_with_lifecycle(ClientConfig::new(ClientCapabilities::default(),Implementation::new("scp-modern-test","1")),TokioChildProcess::new(command).unwrap(),ClientLifecycleMode::Discover {preferred_versions:vec![ProtocolVersion::V_2026_07_28]}).await.unwrap();
        let tools = client.list_all_tools().await.unwrap();
        assert_eq!(tools.len(),5);
        assert!(tools.iter().all(|t|t.output_schema.is_some()));
        let fit = client.call_tool(CallToolRequestParams::new("fit_model").with_arguments(json!({"model_kind":"lm","formula":"Reaction ~ Days","data_path":"sleepstudy.csv"}).as_object().unwrap().clone())).await.unwrap();
        let document = fit.structured_content.unwrap();
        let uri = document["resource"]["uri"].as_str().unwrap();
        assert_eq!(client.peer().list_resources(None).await.unwrap().resources.len(),1);
        let read = client.read_resource(ReadResourceRequestParams::new(uri)).await.unwrap();
        let ResourceContents::TextResourceContents {text,mime_type,..} = &read.contents[0] else {panic!("expected JSON text")};
        assert_eq!(serde_json::from_str::<serde_json::Value>(text).unwrap(),document);
        assert_eq!(mime_type.as_deref(),Some("application/vnd.lme-rs.fit+json"));
        assert!(client.read_resource(ReadResourceRequestParams::new("lme-rs://fits/missing")).await.is_err());
        assert!(client.call_tool(CallToolRequestParams::new("missing")).await.is_err());
        let invalid = client.call_tool(CallToolRequestParams::new("assess_model").with_arguments(json!({"fit_id":"missing"}).as_object().unwrap().clone())).await.unwrap();
        assert_eq!(invalid.is_error,Some(true));
        assert_eq!(client.list_all_tools().await.unwrap().len(),5);
        client.cancel().await.unwrap();
    }).await.expect("modern SCP workflow timed out");
}
