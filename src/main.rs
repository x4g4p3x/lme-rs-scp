use anyhow::Result;
use lme_rs_scp::LmeScpServer;
use rmcp::{transport, ServiceExt};

#[tokio::main]
async fn main() -> Result<()> {
    let service = LmeScpServer::new().serve(transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
