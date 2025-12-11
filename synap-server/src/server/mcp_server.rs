use super::{AppState, get_mcp_tools, handle_mcp_tool};
use crate::config::McpConfig;
use rmcp::model::{
    CallToolRequestParam, CallToolResult, ErrorData, Implementation, ProtocolVersion,
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
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation {
                name: "synap-server".to_string(),
                title: Some("Synap - High-Performance Data Platform".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                website_url: Some("https://github.com/hivellm/synap".to_string()),
                icons: None,
            },
            instructions: Some("Synap MCP Server - High-performance key-value store, message queues, event streams (Kafka-style partitioned topics with consumer groups), and pub/sub messaging.".to_string()),
        }
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParam>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ListToolsResult, ErrorData> {
        use rmcp::model::ListToolsResult;

        let tools = get_mcp_tools(&self.mcp_config);

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        handle_mcp_tool(request, self.state.clone()).await
    }
}
