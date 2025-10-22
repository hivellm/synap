use std::sync::Arc;
use rmcp::model::{CallToolRequestParam, CallToolResult, ErrorData, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use super::{AppState, get_mcp_tools, handle_mcp_tool};

/// MCP Service implementation
#[derive(Clone)]
pub struct SynapMcpService {
    pub state: Arc<AppState>,
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

    fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParam>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<
        Output = Result<rmcp::model::ListToolsResult, ErrorData>,
    > + Send
    + '_ {
        async move {
            use rmcp::model::ListToolsResult;

            let tools = get_mcp_tools();

            Ok(ListToolsResult {
                tools,
                next_cursor: None,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<
        Output = Result<CallToolResult, ErrorData>,
    > + Send
    + '_ {
        async move {
            handle_mcp_tool(request, self.state.clone()).await
        }
    }
}

