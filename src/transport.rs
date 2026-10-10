//! Legacy-compatible SCP/MCP wire contract, with cancellable background workflows.
use crate::LmeScpServer;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::*;
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData, ServerHandler};

struct ResponseLease {
    response: Option<CallToolResponse>,
    created_uri: Option<String>,
    server: LmeScpServer,
}
impl ResponseLease {
    fn deliver(mut self) -> CallToolResponse {
        self.created_uri = None;
        self.response
            .take()
            .expect("response available until delivery")
    }
}
impl Drop for ResponseLease {
    fn drop(&mut self) {
        if let Some(uri) = &self.created_uri {
            let _ = self.server.forget_resource(uri);
        }
    }
}
impl ServerHandler for LmeScpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
    }
    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tool_router.get(name).cloned()
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(self.tool_router.list_all()))
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        if self.tool_router.get(&request.name).is_none() {
            return Err(ErrorData::new(
                ErrorCode::METHOD_NOT_FOUND,
                "unknown scientific workflow",
                None,
            ));
        }
        let (tx, rx) = tokio::sync::oneshot::channel();
        let server = self.clone();
        let worker_context = context.clone();
        // Keep supervising the blocking worker after its caller is cancelled.
        tokio::spawn(async move {
            if tx.is_closed() {
                return;
            }
            let produces_resource = matches!(
                request.name.as_ref(),
                "fit_model" | "assess_model" | "infer_marginals"
            );
            let response = server
                .tool_router
                .call(ToolCallContext::new(&server, request, worker_context))
                .await
                // Workflow failures are tool results; unknown methods remain RPC errors.
                .or_else(|error| {
                    Ok::<_, ErrorData>(
                        CallToolResult::error(vec![ContentBlock::text(error.message.to_string())])
                            .into(),
                    )
                })
                .map(|response| {
                    let created_uri = if produces_resource {
                        match &response {
                            CallToolResponse::Complete(result) => result
                                .structured_content
                                .as_ref()
                                .and_then(|v| v.get("resource"))
                                .and_then(|v| v.get("uri"))
                                .and_then(|v| v.as_str())
                                .map(str::to_owned),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    ResponseLease {
                        response: Some(response),
                        created_uri,
                        server: server.clone(),
                    }
                });
            let _ = tx.send(response);
        });
        tokio::select! {
            biased;
            _ = context.ct.cancelled() => Err(ErrorData::internal_error("workflow request cancelled",None)),
            response = rx => Ok(response.map_err(|_|ErrorData::internal_error("workflow supervisor failed",None))??.deliver()),
        }
    }
    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        if request.is_some_and(|p| p.cursor.is_some()) {
            return Err(ErrorData::invalid_params(
                "resource list has no continuation cursor",
                None,
            ));
        }
        Ok(ListResourcesResult::with_all_items(
            self.resources()
                .into_iter()
                .map(|r| Resource::new(r.uri, r.name).with_mime_type(r.media_type))
                .collect(),
        ))
    }
    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        let record = LmeScpServer::read_resource(self, &request.uri)
            .map_err(crate::server::to_protocol_error)?;
        let text = serde_json::to_string(&record.document)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        let mut content = ResourceContents::text(text, record.uri);
        if let ResourceContents::TextResourceContents { mime_type, .. } = &mut content {
            *mime_type = Some(record.media_type);
        }
        Ok(ReadResourceResult::new(vec![content]).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::ServiceExt;
    use std::sync::{Arc, Barrier};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancelled_wire_request_keeps_capacity_and_cleans_its_fit() {
        tokio::time::timeout(std::time::Duration::from_secs(30),async {
            let server = LmeScpServer::new();
            let started = Arc::new(Barrier::new(2));
            let release = Arc::new(Barrier::new(2));
            *server.test_gate.lock().unwrap() = Some((started.clone(),release.clone()));
            let (server_io,client_io) = tokio::io::duplex(65536);
            let serving = server.clone();
            let service = tokio::spawn(async move {serving.serve(server_io).await.unwrap().waiting().await.unwrap();});
            let client = ().serve(client_io).await.unwrap();
            let args = serde_json::json!({"model_kind":"lm","formula":"Reaction ~ Days","data_path":std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/sleepstudy.csv")}).as_object().unwrap().clone();
            let handle = client.peer().send_cancellable_request(ClientRequest::CallToolRequest(Request::new(CallToolRequestParams::new("fit_model").with_arguments(args.clone()))),rmcp::service::PeerRequestOptions::no_options()).await.unwrap();
            tokio::task::spawn_blocking(move ||started.wait()).await.unwrap();
            client.peer().notify_cancelled(CancelledNotificationParam::new(Some(handle.id.clone()),Some("regression cancellation".into()))).await.unwrap();
            assert_eq!(client.list_all_tools().await.unwrap().len(),5);
            assert_eq!(server.jobs.available_permits(),1);
            let occupied = server.jobs.clone().try_acquire_owned().unwrap();
            assert_eq!(client.call_tool(CallToolRequestParams::new("fit_model").with_arguments(args)).await.unwrap().is_error,Some(true));
            drop(occupied);
            tokio::task::spawn_blocking(move ||release.wait()).await.unwrap();
            loop {
                if server.jobs.available_permits()==2 && server.resources().is_empty() && server.api().list_models().fits.is_empty() {break;}
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
            drop(handle);
            client.cancel().await.unwrap();
            service.abort();
        }).await.expect("cancellation regression timed out");
    }
    #[test]
    fn abandoned_response_removes_resource_and_fit() {
        let server = LmeScpServer::new();
        let request = serde_json::from_value(serde_json::json!({"model_kind":"lm","formula":"Reaction ~ Days","data_path":std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/sleepstudy.csv")})).unwrap();
        let result = server.fit_model(request).unwrap();
        drop(ResponseLease {
            response: None,
            created_uri: Some(result.resource.uri),
            server: server.clone(),
        });
        assert!(server.resources().is_empty());
        assert!(server.api().list_models().fits.is_empty());
    }
}
