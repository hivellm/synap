use super::{AppState, get_mcp_tools, handle_mcp_tool};
use crate::config::McpConfig;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, ErrorData, Implementation, ProtocolVersion,
    ServerCapabilities, ServerInfo,
};
use std::sync::Arc;

/// MCP Service implementation
#[derive(Clone)]
pub struct SynapMcpService {
    pub state: Arc<AppState>,
    pub mcp_config: McpConfig,
}

impl rmcp::ServerHandler for SynapMcpService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .build(),
        )
        .with_protocol_version(ProtocolVersion::default())
        .with_server_info(
            Implementation::new("synap-server", env!("CARGO_PKG_VERSION"))
                .with_title("Synap - High-Performance Data Platform")
                .with_website_url("https://github.com/hivellm/synap"),
        )
        .with_instructions("Synap MCP Server - High-performance key-value store, message queues, event streams (Kafka-style partitioned topics with consumer groups), and pub/sub messaging.")
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ListToolsResult, ErrorData> {
        use rmcp::model::ListToolsResult;

        let tools = get_mcp_tools(&self.mcp_config);

        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        // Synap tools always run to completion in-process: no elicitation, no
        // long-running task handles, so every reply is a `Complete` response.
        handle_mcp_tool(request, self.state.clone())
            .await
            .map(CallToolResponse::from)
    }
}
