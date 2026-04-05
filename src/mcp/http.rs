use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::Value;

use crate::mcp::protocol::{JsonRpcRequest, JsonRpcResponse};

pub struct HttpTransport {
    client: Client,
    base_url: String,
    next_id: AtomicU64,
    session_id: Option<String>,
    headers: HashMap<String, String>,
}

impl HttpTransport {
    pub fn new(base_url: &str, headers: HashMap<String, String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            next_id: AtomicU64::new(1),
            session_id: None,
            headers,
        })
    }

    pub async fn send_request(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<JsonRpcResponse> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let mut req_builder = self
            .client
            .post(&self.base_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream");

        if let Some(ref session_id) = self.session_id {
            req_builder = req_builder.header("Mcp-Session-Id", session_id);
        }

        for (key, value) in &self.headers {
            req_builder = req_builder.header(key, value);
        }

        let body =
            serde_json::to_string(&request).context("Failed to serialize JSON-RPC request")?;

        let response = req_builder
            .body(body)
            .send()
            .await
            .with_context(|| format!("Failed to send request to {}", self.base_url))?;

        if let Some(session_id) = response.headers().get("Mcp-Session-Id") {
            self.session_id = Some(
                session_id
                    .to_str()
                    .context("Invalid Mcp-Session-Id header")?
                    .to_string(),
            );
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        if content_type.contains("text/event-stream") {
            self.parse_sse_response(response).await
        } else {
            let text = response
                .text()
                .await
                .context("Failed to read response body")?;
            serde_json::from_str(&text)
                .with_context(|| format!("Failed to parse JSON-RPC response: {}", text))
        }
    }

    async fn parse_sse_response(&self, response: reqwest::Response) -> Result<JsonRpcResponse> {
        let text = response
            .text()
            .await
            .context("Failed to read SSE response")?;

        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(data) {
                    return Ok(response);
                }
            }
        }

        anyhow::bail!("No valid JSON-RPC response found in SSE stream")
    }

    pub async fn send_notification(&mut self, method: &str, params: Option<Value>) -> Result<()> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params.unwrap_or(Value::Null)
        });

        let mut req_builder = self
            .client
            .post(&self.base_url)
            .header("Content-Type", "application/json");

        if let Some(ref session_id) = self.session_id {
            req_builder = req_builder.header("Mcp-Session-Id", session_id);
        }

        for (key, value) in &self.headers {
            req_builder = req_builder.header(key, value);
        }

        req_builder
            .body(serde_json::to_string(&notification)?)
            .send()
            .await
            .context("Failed to send notification")?;

        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        let _ = self
            .send_notification("notifications/cancelled", None)
            .await;
        Ok(())
    }

    pub fn set_auth_token(&mut self, token: &str) {
        self.headers
            .insert("Authorization".to_string(), token.to_string());
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_http_transport_creation() {
        let headers = HashMap::new();
        let transport = HttpTransport::new("http://localhost:3000", headers);
        assert!(transport.is_ok());
        let transport = transport.unwrap();
        assert_eq!(transport.base_url, "http://localhost:3000");
        assert_eq!(transport.next_id.load(Ordering::SeqCst), 1);
        assert!(transport.session_id.is_none());
    }

    #[test]
    fn test_http_transport_creation_with_trailing_slash() {
        let headers = HashMap::new();
        let transport = HttpTransport::new("http://localhost:3000/", headers).unwrap();
        assert_eq!(transport.base_url, "http://localhost:3000");
    }

    #[test]
    fn test_http_request_format() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: 42,
            method: "tools/list".to_string(),
            params: Some(json!({"foo": "bar"})),
        };

        let serialized = serde_json::to_string(&request).unwrap();
        let value: serde_json::Value = serde_json::from_str(&serialized).unwrap();

        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(value["id"], 42);
        assert_eq!(value["method"], "tools/list");
        assert_eq!(value["params"]["foo"], "bar");
    }

    #[test]
    fn test_sse_response_parsing() {
        let headers = HashMap::new();
        let transport = HttpTransport::new("http://localhost:3000", headers).unwrap();

        let sse_text = "data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"tools\":[]}}\n\n";
        let lines: Vec<&str> = sse_text.lines().collect();

        let mut found = false;
        for line in lines {
            if let Some(data) = line.strip_prefix("data: ") {
                if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(data) {
                    assert_eq!(response.jsonrpc, "2.0");
                    assert_eq!(response.id, Some(1));
                    assert!(response.result.is_some());
                    found = true;
                }
            }
        }
        assert!(found, "Should have parsed SSE data line");
    }

    #[test]
    fn test_session_id_tracking() {
        let headers = HashMap::new();
        let mut transport = HttpTransport::new("http://localhost:3000", headers).unwrap();

        assert!(transport.session_id.is_none());
        transport.session_id = Some("test-session-123".to_string());
        assert_eq!(transport.session_id, Some("test-session-123".to_string()));
    }

    #[test]
    fn test_http_transport_with_custom_headers() {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer token123".to_string());
        headers.insert("X-Custom-Header".to_string(), "custom-value".to_string());

        let transport = HttpTransport::new("http://localhost:3000", headers).unwrap();
        assert_eq!(transport.headers.len(), 2);
        assert_eq!(
            transport.headers.get("Authorization"),
            Some(&"Bearer token123".to_string())
        );
        assert_eq!(
            transport.headers.get("X-Custom-Header"),
            Some(&"custom-value".to_string())
        );
    }

    #[test]
    fn test_notification_format() {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": json!({})
        });

        assert_eq!(notification["jsonrpc"], "2.0");
        assert_eq!(notification["method"], "notifications/initialized");
        assert!(notification["params"].is_object());
        assert!(notification.get("id").is_none());
    }

    #[test]
    fn test_set_auth_token() {
        let headers = HashMap::new();
        let mut transport = HttpTransport::new("http://localhost:3000", headers).unwrap();

        assert!(transport.headers.get("Authorization").is_none());

        transport.set_auth_token("Bearer test_token_123");
        assert_eq!(
            transport.headers.get("Authorization"),
            Some(&"Bearer test_token_123".to_string())
        );

        transport.set_auth_token("Bearer new_token_456");
        assert_eq!(
            transport.headers.get("Authorization"),
            Some(&"Bearer new_token_456".to_string())
        );
    }
}
