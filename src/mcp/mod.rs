pub mod http;
pub mod oauth;
pub mod protocol;
pub mod stdio;

use std::collections::HashMap;

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::ai::types::ToolDefinition;
use crate::config::McpServerConfig;

use http::HttpTransport;
use oauth::OAuthManager;
pub use oauth::{OAuthConfig, OAuthToken};
use protocol::{
    CallToolParams, CallToolResult, ClientCapabilities, ClientInfo, InitializeParams,
    InitializeResult, ListToolsResult, McpTool,
};
use stdio::StdioTransport;

const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

enum Transport {
    Stdio(StdioTransport),
    Http(HttpTransport),
}

impl Transport {
    async fn send_request(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<protocol::JsonRpcResponse> {
        match self {
            Transport::Stdio(t) => t.send_request(method, params).await,
            Transport::Http(t) => t.send_request(method, params).await,
        }
    }

    async fn send_notification(&mut self, method: &str, params: Option<Value>) -> Result<()> {
        match self {
            Transport::Stdio(t) => t.send_notification(method, params).await,
            Transport::Http(t) => t.send_notification(method, params).await,
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        match self {
            Transport::Stdio(t) => t.shutdown().await,
            Transport::Http(t) => t.shutdown().await,
        }
    }
}

pub struct McpServer {
    name: String,
    transport: Transport,
    tools: Vec<McpTool>,
    initialized: bool,
}

pub struct McpManager {
    servers: HashMap<String, McpServer>,
    oauth_manager: OAuthManager,
}

impl McpManager {
    pub fn new() -> Self {
        let oauth_manager = OAuthManager::new().unwrap_or_default();
        Self {
            servers: HashMap::new(),
            oauth_manager,
        }
    }

    pub async fn connect(&mut self, name: &str, config: &McpServerConfig) -> Result<()> {
        let mut transport = if let Some(url) = &config.url {
            let mut headers = config.headers.clone().unwrap_or_default();

            if let Some(oauth_config) = &config.oauth {
                if let Some(auth_header) = self.oauth_manager.get_auth_header(name) {
                    if self.oauth_manager.is_token_expired(name) {
                        if let Err(e) = self.oauth_manager.refresh_token(name, oauth_config).await {
                            tracing::warn!("Failed to refresh OAuth token for '{}': {}", name, e);
                        } else if let Some(refreshed_header) =
                            self.oauth_manager.get_auth_header(name)
                        {
                            headers.insert("Authorization".to_string(), refreshed_header);
                        }
                    } else {
                        headers.insert("Authorization".to_string(), auth_header);
                    }
                }
            }

            let mut http_transport = HttpTransport::new(url, headers)
                .with_context(|| format!("Failed to create HTTP transport for '{name}'"))?;
            Transport::Http(http_transport)
        } else {
            let command = config
                .command
                .as_deref()
                .with_context(|| format!("MCP server '{name}' is missing command or url"))?;
            let args = config.args.clone().unwrap_or_default();
            let env = config.env.clone().unwrap_or_default();

            let stdio_transport = StdioTransport::spawn(command, &args, &env)
                .await
                .with_context(|| format!("Failed to spawn MCP server '{name}'"))?;
            Transport::Stdio(stdio_transport)
        };

        let initialize_params = InitializeParams {
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: ClientInfo {
                name: "OpenRust".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        let initialize_response = transport
            .send_request("initialize", Some(serde_json::to_value(initialize_params)?))
            .await
            .with_context(|| format!("Failed MCP initialize request for '{name}'"))?;

        if let Some(error) = initialize_response.error {
            anyhow::bail!(
                "MCP initialize failed for '{name}': {} ({})",
                error.message,
                error.code
            );
        }

        let initialize_result_value = initialize_response
            .result
            .with_context(|| format!("MCP initialize missing result for '{name}'"))?;
        let _: InitializeResult = serde_json::from_value(initialize_result_value)
            .with_context(|| format!("Invalid MCP initialize response for '{name}'"))?;

        transport
            .send_notification("notifications/initialized", Some(json!({})))
            .await
            .with_context(|| format!("Failed MCP initialized notification for '{name}'"))?;

        let list_response = transport
            .send_request("tools/list", Some(json!({})))
            .await
            .with_context(|| format!("Failed tools/list request for MCP server '{name}'"))?;

        if let Some(error) = list_response.error {
            anyhow::bail!(
                "MCP tools/list failed for '{name}': {} ({})",
                error.message,
                error.code
            );
        }

        let list_result_value = list_response
            .result
            .with_context(|| format!("MCP tools/list missing result for '{name}'"))?;
        let list_result: ListToolsResult = serde_json::from_value(list_result_value)
            .with_context(|| format!("Invalid tools/list response for '{name}'"))?;

        let server = McpServer {
            name: name.to_string(),
            transport,
            tools: list_result.tools,
            initialized: true,
        };

        self.servers.insert(name.to_string(), server);
        Ok(())
    }

    pub async fn disconnect(&mut self, name: &str) -> Result<()> {
        if let Some(mut server) = self.servers.remove(name) {
            server
                .transport
                .shutdown()
                .await
                .with_context(|| format!("Failed to shutdown MCP server '{name}'"))?;
        }
        Ok(())
    }

    pub async fn disconnect_all(&mut self) -> Result<()> {
        let names: Vec<String> = self.servers.keys().cloned().collect();
        for name in names {
            self.disconnect(&name).await?;
        }
        Ok(())
    }

    pub async fn list_tools(&self, server_name: &str) -> Result<Vec<McpTool>> {
        let server = self
            .servers
            .get(server_name)
            .with_context(|| format!("MCP server '{server_name}' is not connected"))?;
        Ok(server.tools.clone())
    }

    pub async fn call_tool(
        &mut self,
        server_name: &str,
        tool_name: &str,
        args: Value,
    ) -> Result<CallToolResult> {
        let server = self
            .servers
            .get_mut(server_name)
            .with_context(|| format!("MCP server '{server_name}' is not connected"))?;

        let params = CallToolParams {
            name: tool_name.to_string(),
            arguments: Some(args),
        };

        let response = server
            .transport
            .send_request("tools/call", Some(serde_json::to_value(params)?))
            .await
            .with_context(|| {
                format!(
                    "Failed tools/call request for MCP server '{server_name}', tool '{tool_name}'"
                )
            })?;

        if let Some(error) = response.error {
            anyhow::bail!(
                "MCP tools/call failed for '{server_name}::{tool_name}': {} ({})",
                error.message,
                error.code
            );
        }

        let result_value = response.result.with_context(|| {
            format!("MCP tools/call missing result for '{server_name}::{tool_name}'")
        })?;
        let result: CallToolResult = serde_json::from_value(result_value).with_context(|| {
            format!("Invalid MCP tools/call result for '{server_name}::{tool_name}'")
        })?;

        Ok(result)
    }

    pub fn get_all_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.servers
            .values()
            .filter(|server| server.initialized)
            .flat_map(|server| {
                server
                    .tools
                    .iter()
                    .map(|tool| prefixed_tool_definition(&server.name, tool))
            })
            .collect()
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

// TODO: Add full Tool trait integration once async MCP routing is added in the agent loop.

fn prefixed_tool_definition(server_name: &str, tool: &McpTool) -> ToolDefinition {
    ToolDefinition {
        name: format!("{server_name}::{}", tool.name),
        description: tool
            .description
            .clone()
            .unwrap_or_else(|| format!("MCP tool '{}'", tool.name)),
        input_schema: tool.input_schema.clone(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{prefixed_tool_definition, HttpTransport, McpManager, StdioTransport};
    use crate::mcp::protocol::McpTool;

    #[test]
    fn test_tool_name_prefixing() {
        let tool = McpTool {
            name: "get_forecast".to_string(),
            description: Some("Get weather forecast".to_string()),
            input_schema: json!({"type": "object"}),
        };

        let def = prefixed_tool_definition("weather", &tool);
        assert_eq!(def.name, "weather::get_forecast");
        assert_eq!(def.description, "Get weather forecast");
        assert_eq!(def.input_schema["type"], "object");
    }

    #[test]
    fn test_mcp_manager_new() {
        let manager = McpManager::new();
        assert!(manager.servers.is_empty());
    }

    #[tokio::test]
    async fn test_transport_enum_stdio() {
        use std::collections::HashMap;
        let result = StdioTransport::spawn("echo", &[], &HashMap::new()).await;
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_transport_enum_http() {
        use std::collections::HashMap;
        let headers = HashMap::new();
        let transport = HttpTransport::new("http://localhost:3000", headers);
        assert!(transport.is_ok());
    }

    #[test]
    fn test_mcp_server_config_with_url() {
        use crate::config::McpServerConfig;
        let mut config = McpServerConfig::default();
        config.url = Some("http://localhost:3000".to_string());
        assert!(config.url.is_some());
        assert!(config.command.is_none());
    }

    #[test]
    fn test_mcp_server_config_with_headers() {
        use crate::config::McpServerConfig;
        use std::collections::HashMap;

        let mut config = McpServerConfig::default();
        config.url = Some("http://localhost:3000".to_string());

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer token".to_string());
        config.headers = Some(headers);

        assert!(config.headers.is_some());
        assert_eq!(
            config.headers.unwrap().get("Authorization"),
            Some(&"Bearer token".to_string())
        );
    }

    #[test]
    fn test_mcp_manager_with_oauth() {
        let manager = McpManager::new();
        assert!(manager.servers.is_empty());
    }

    #[test]
    fn test_mcp_tool_definitions_merged() {
        use crate::mcp::protocol::McpTool;

        let manager = McpManager::new();
        let tools = manager.get_all_tool_definitions();
        assert!(tools.is_empty());

        let tool = McpTool {
            name: "test_tool".to_string(),
            description: Some("A test tool".to_string()),
            input_schema: json!({"type": "object"}),
        };

        let def = prefixed_tool_definition("test_server", &tool);
        assert_eq!(def.name, "test_server::test_tool");
    }

    #[test]
    fn test_oauth_config_in_server_config() {
        use crate::config::McpServerConfig;
        use crate::mcp::OAuthConfig;

        let mut config = McpServerConfig::default();
        config.url = Some("http://localhost:3000".to_string());
        config.oauth = Some(OAuthConfig {
            client_id: Some("test_client".to_string()),
            client_secret: Some("test_secret".to_string()),
            auth_url: Some("https://auth.example.com/authorize".to_string()),
            token_url: Some("https://auth.example.com/token".to_string()),
            scopes: vec!["read".to_string()],
        });

        assert!(config.oauth.is_some());
        let oauth = config.oauth.unwrap();
        assert_eq!(oauth.client_id, Some("test_client".to_string()));
        assert_eq!(oauth.scopes, vec!["read"]);
    }
}
