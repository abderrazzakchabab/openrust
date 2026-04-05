use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<u64>,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    pub client_info: ClientInfo,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ClientCapabilities {}

#[derive(Debug, Clone, Serialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: Option<ServerInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerCapabilities {
    pub tools: Option<ToolsCapability>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCapability {
    #[serde(default)]
    pub list_changed: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    #[serde(default, rename = "inputSchema")]
    pub input_schema: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListToolsResult {
    pub tools: Vec<McpTool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CallToolParams {
    pub name: String,
    pub arguments: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    pub content: Vec<ToolContent>,
    #[serde(default)]
    pub is_error: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: Option<String>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{CallToolResult, InitializeParams, JsonRpcRequest, JsonRpcResponse, McpTool};
    use super::{ClientCapabilities, ClientInfo};

    #[test]
    fn test_json_rpc_request_serialization() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: 7,
            method: "tools/list".to_string(),
            params: Some(json!({ "cursor": "abc" })),
        };

        let value = serde_json::to_value(req).expect("serialize json-rpc request");
        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(value["id"], 7);
        assert_eq!(value["method"], "tools/list");
        assert_eq!(value["params"]["cursor"], "abc");
    }

    #[test]
    fn test_json_rpc_response_parsing() {
        let raw = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "tools": [] }
        });

        let response: JsonRpcResponse =
            serde_json::from_value(raw).expect("parse json-rpc response");
        assert_eq!(response.jsonrpc, "2.0");
        assert_eq!(response.id, Some(1));
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_json_rpc_error_parsing() {
        let raw = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32601,
                "message": "Method not found",
                "data": { "method": "tools/x" }
            }
        });

        let response: JsonRpcResponse = serde_json::from_value(raw).expect("parse error response");
        let error = response.error.expect("error field exists");
        assert_eq!(error.code, -32601);
        assert_eq!(error.message, "Method not found");
        assert_eq!(error.data.expect("error data")["method"], "tools/x");
    }

    #[test]
    fn test_initialize_params_serialization() {
        let params = InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: ClientInfo {
                name: "OpenRust".to_string(),
                version: "0.1.0".to_string(),
            },
        };

        let value = serde_json::to_value(params).expect("serialize initialize params");
        assert_eq!(value["protocolVersion"], "2024-11-05");
        assert_eq!(value["clientInfo"]["name"], "OpenRust");
        assert_eq!(value["clientInfo"]["version"], "0.1.0");
        assert!(value["capabilities"].is_object());
    }

    #[test]
    fn test_mcp_tool_parsing() {
        let raw = json!({
            "name": "get_weather",
            "description": "Get weather for a city",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "city": { "type": "string" }
                },
                "required": ["city"]
            }
        });

        let tool: McpTool = serde_json::from_value(raw).expect("parse mcp tool");
        assert_eq!(tool.name, "get_weather");
        assert_eq!(tool.description.as_deref(), Some("Get weather for a city"));
        assert_eq!(tool.input_schema["type"], "object");
    }

    #[test]
    fn test_call_tool_result_parsing() {
        let raw = json!({
            "content": [
                { "type": "text", "text": "72F and sunny" }
            ],
            "isError": false
        });

        let result: CallToolResult = serde_json::from_value(raw).expect("parse call tool result");
        assert!(!result.is_error);
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.content[0].content_type, "text");
        assert_eq!(result.content[0].text.as_deref(), Some("72F and sunny"));
    }
}
