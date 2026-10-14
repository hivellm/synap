use super::{AppState, get_mcp_tools, handle_mcp_tool};
use crate::auth::{AuthContext, OperationContext};
use crate::config::McpConfig;
use crate::hub::HubUserContext;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, ErrorData, Implementation, ProtocolVersion,
    ServerCapabilities, ServerConfig,
};
use std::sync::Arc;

/// MCP Service implementation
#[derive(Clone)]
pub struct SynapMcpService {
    pub state: Arc<AppState>,
    pub mcp_config: McpConfig,
    /// Whether authentication is enabled: then a request that reaches a tool
    /// without an identity is denied instead of running unrestricted.
    pub auth_enabled: bool,
}

impl SynapMcpService {
    /// The identity of the HTTP request behind this MCP call. rmcp hands every
    /// request's `http::request::Parts` to the handler, and the `/mcp` auth
    /// layer put the request's `AuthContext` (and, in Hub mode, its
    /// `HubUserContext`) in their extensions, so each call is judged and
    /// scoped by its own credentials — never by ambient state.
    fn caller(
        &self,
        context: &rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<OperationContext, ErrorData> {
        let parts = context.extensions.get::<axum::http::request::Parts>();
        let auth = parts.and_then(|parts| parts.extensions.get::<AuthContext>());
        let tenant = parts
            .and_then(|parts| parts.extensions.get::<HubUserContext>())
            .map(|hub| hub.user_id);
        match auth {
            Some(auth) => Ok(OperationContext::from_auth_context(auth).with_tenant(tenant)),
            None if !self.auth_enabled => Ok(OperationContext::unrestricted()),
            None => Err(ErrorData::invalid_request(
                "NOPERM the request carries no identity",
                None,
            )),
        }
    }
}

impl rmcp::ServerHandler for SynapMcpService {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
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
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let caller = self.caller(&context)?;
        // Synap tools always run to completion in-process: no elicitation, no
        // long-running task handles, so every reply is a `Complete` response.
        handle_mcp_tool(request, self.state.clone(), &caller)
            .await
            .map(CallToolResponse::from)
    }
}
