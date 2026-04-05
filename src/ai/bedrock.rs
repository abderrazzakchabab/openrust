use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use futures::StreamExt;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

use super::types::{
    AiError, CompletionRequest, CompletionResponse, ContentBlock, ContentDelta, Message, Role,
    StreamEvent, ToolDefinition, Usage,
};
use super::Provider;

pub struct BedrockProvider {
    region: String,
    access_key: String,
    secret_key: String,
    session_token: Option<String>,
    client: Client,
    model_id: String,
}

impl BedrockProvider {
    pub fn new(
        region: String,
        access_key: String,
        secret_key: String,
        session_token: Option<String>,
        model_id: String,
    ) -> Self {
        BedrockProvider {
            region,
            access_key,
            secret_key,
            session_token,
            client: Client::new(),
            model_id,
        }
    }

    fn build_url(&self, streaming: bool) -> String {
        let action = if streaming {
            "converse-stream"
        } else {
            "converse"
        };
        format!(
            "https://bedrock-runtime.{}.amazonaws.com/model/{}/{}",
            self.region, self.model_id, action
        )
    }

    fn sign_request(
        &self,
        method: &str,
        url: &str,
        headers: &HashMap<String, String>,
        payload: &str,
    ) -> Result<HashMap<String, String>, AiError> {
        let datetime = Utc::now();
        let amz_date = datetime.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = datetime.format("%Y%m%d").to_string();

        let payload_hash = hex::encode(Sha256::digest(payload.as_bytes()));

        let mut canonical_headers_map = headers.clone();
        canonical_headers_map.insert("host".to_string(), format!("bedrock-runtime.{}.amazonaws.com", self.region));
        canonical_headers_map.insert("x-amz-date".to_string(), amz_date.clone());
        canonical_headers_map.insert("x-amz-content-sha256".to_string(), payload_hash.clone());

        if let Some(token) = &self.session_token {
            canonical_headers_map.insert("x-amz-security-token".to_string(), token.clone());
        }

        let mut header_keys: Vec<String> = canonical_headers_map.keys().cloned().collect();
        header_keys.sort();

        let canonical_headers = header_keys
            .iter()
            .map(|key| format!("{}:{}", key.to_lowercase(), canonical_headers_map[key].trim()))
            .collect::<Vec<_>>()
            .join("\n");

        let signed_headers = header_keys
            .iter()
            .map(|key| key.to_lowercase())
            .collect::<Vec<_>>()
            .join(";");

        let url_parts: Vec<&str> = url.split('?').collect();
        let canonical_uri = url_parts[0]
            .strip_prefix(&format!("https://bedrock-runtime.{}.amazonaws.com", self.region))
            .unwrap_or("/");
        let canonical_querystring = url_parts.get(1).unwrap_or(&"");

        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n\n{}\n{}",
            method, canonical_uri, canonical_querystring, canonical_headers, signed_headers, payload_hash
        );

        let credential_scope = format!("{}/bedrock/aws4_request", date_stamp);
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            amz_date,
            credential_scope,
            hex::encode(Sha256::digest(canonical_request.as_bytes()))
        );

        let k_date = hmac_sha256(format!("AWS4{}", self.secret_key).as_bytes(), date_stamp.as_bytes());
        let k_region = hmac_sha256(&k_date, self.region.as_bytes());
        let k_service = hmac_sha256(&k_region, b"bedrock");
        let k_signing = hmac_sha256(&k_service, b"aws4_request");
        let signature = hex::encode(hmac_sha256(&k_signing, string_to_sign.as_bytes()));

        let authorization_header = format!(
            "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
            self.access_key, credential_scope, signed_headers, signature
        );

        let mut signed_headers_map = HashMap::new();
        signed_headers_map.insert("Authorization".to_string(), authorization_header);
        signed_headers_map.insert("x-amz-date".to_string(), amz_date);
        signed_headers_map.insert("x-amz-content-sha256".to_string(), payload_hash);
        signed_headers_map.insert("host".to_string(), format!("bedrock-runtime.{}.amazonaws.com", self.region));

        if let Some(token) = &self.session_token {
            signed_headers_map.insert("x-amz-security-token".to_string(), token.clone());
        }

        Ok(signed_headers_map)
    }
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

#[derive(Debug, Clone, Serialize)]
struct BedrockRequest {
    #[serde(rename = "modelId")]
    model_id: String,
    messages: Vec<BedrockMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<Vec<BedrockSystemBlock>>,
    #[serde(rename = "inferenceConfig")]
    inference_config: BedrockInferenceConfig,
    #[serde(skip_serializing_if = "Option::is_none", rename = "toolConfig")]
    tool_config: Option<BedrockToolConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BedrockSystemBlock {
    text: String,
}

#[derive(Debug, Clone, Serialize)]
struct BedrockInferenceConfig {
    #[serde(rename = "maxTokens")]
    max_tokens: u32,
}

#[derive(Debug, Clone, Serialize)]
struct BedrockToolConfig {
    tools: Vec<BedrockToolSpec>,
}

#[derive(Debug, Clone, Serialize)]
struct BedrockToolSpec {
    #[serde(rename = "toolSpec")]
    tool_spec: BedrockToolDefinition,
}

#[derive(Debug, Clone, Serialize)]
struct BedrockToolDefinition {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: BedrockInputSchema,
}

#[derive(Debug, Clone, Serialize)]
struct BedrockInputSchema {
    json: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BedrockMessage {
    role: String,
    content: Vec<BedrockContentBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum BedrockContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        #[serde(rename = "toolUse")]
        tool_use: BedrockToolUse,
    },
    ToolResult {
        #[serde(rename = "toolResult")]
        tool_result: BedrockToolResult,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BedrockToolUse {
    #[serde(rename = "toolUseId")]
    tool_use_id: String,
    name: String,
    input: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BedrockToolResult {
    #[serde(rename = "toolUseId")]
    tool_use_id: String,
    content: Vec<BedrockToolResultContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BedrockToolResultContent {
    text: String,
}

#[derive(Debug, Deserialize)]
struct BedrockResponse {
    output: BedrockOutput,
    #[serde(rename = "stopReason")]
    stop_reason: Option<String>,
    usage: Option<BedrockUsage>,
}

#[derive(Debug, Deserialize)]
struct BedrockOutput {
    message: BedrockMessage,
}

#[derive(Debug, Deserialize)]
struct BedrockUsage {
    #[serde(rename = "inputTokens")]
    input_tokens: Option<u32>,
    #[serde(rename = "outputTokens")]
    output_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum BedrockStreamEvent {
    #[serde(rename = "messageStart")]
    MessageStart,
    #[serde(rename = "contentBlockStart")]
    ContentBlockStart {
        #[serde(rename = "contentBlockIndex")]
        content_block_index: u32,
        start: BedrockContentBlockStart,
    },
    #[serde(rename = "contentBlockDelta")]
    ContentBlockDelta {
        #[serde(rename = "contentBlockIndex")]
        content_block_index: u32,
        delta: BedrockDelta,
    },
    #[serde(rename = "contentBlockStop")]
    ContentBlockStop {
        #[serde(rename = "contentBlockIndex")]
        content_block_index: u32,
    },
    #[serde(rename = "messageStop")]
    MessageStop {
        #[serde(rename = "stopReason")]
        stop_reason: Option<String>,
    },
    #[serde(rename = "metadata")]
    Metadata { usage: Option<BedrockUsage> },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BedrockContentBlockStart {
    ToolUse {
        #[serde(rename = "toolUse")]
        tool_use: BedrockToolUse,
    },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BedrockDelta {
    Text { text: String },
    ToolUse { #[serde(rename = "toolUse")] tool_use: BedrockToolUseDelta },
}

#[derive(Debug, Deserialize)]
struct BedrockToolUseDelta {
    input: String,
}

fn convert_tools(tools: Option<&[ToolDefinition]>) -> Option<BedrockToolConfig> {
    tools.map(|definitions| BedrockToolConfig {
        tools: definitions
            .iter()
            .map(|tool| BedrockToolSpec {
                tool_spec: BedrockToolDefinition {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                    input_schema: BedrockInputSchema {
                        json: tool.input_schema.clone(),
                    },
                },
            })
            .collect(),
    })
}

fn convert_messages(messages: &[Message]) -> Vec<BedrockMessage> {
    messages
        .iter()
        .filter(|message| message.role != Role::System)
        .map(|message| BedrockMessage {
            role: message.role.to_string(),
            content: message
                .content
                .iter()
                .map(|block| match block {
                    ContentBlock::Text { text } => BedrockContentBlock::Text { text: text.clone() },
                    ContentBlock::ToolUse { id, name, input } => BedrockContentBlock::ToolUse {
                        tool_use: BedrockToolUse {
                            tool_use_id: id.clone(),
                            name: name.clone(),
                            input: input.clone(),
                        },
                    },
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => BedrockContentBlock::ToolResult {
                        tool_result: BedrockToolResult {
                            tool_use_id: tool_use_id.clone(),
                            content: vec![BedrockToolResultContent {
                                text: content.clone(),
                            }],
                            status: if *is_error {
                                Some("error".to_string())
                            } else {
                                Some("success".to_string())
                            },
                        },
                    },
                    ContentBlock::Thinking { .. } => BedrockContentBlock::Text {
                        text: String::new(),
                    },
                })
                .collect(),
        })
        .collect()
}

fn get_system_prompt(messages: &[Message], system: Option<&str>) -> Option<Vec<BedrockSystemBlock>> {
    let system_text = system
        .map(String::from)
        .or_else(|| {
            messages
                .iter()
                .find(|message| message.role == Role::System)
                .map(Message::text_content)
        });

    system_text.map(|text| vec![BedrockSystemBlock { text }])
}

fn convert_response_content(blocks: Vec<BedrockContentBlock>) -> Vec<ContentBlock> {
    blocks
        .into_iter()
        .filter_map(|block| match block {
            BedrockContentBlock::Text { text } => Some(ContentBlock::Text { text }),
            BedrockContentBlock::ToolUse { tool_use } => Some(ContentBlock::ToolUse {
                id: tool_use.tool_use_id,
                name: tool_use.name,
                input: tool_use.input,
            }),
            BedrockContentBlock::ToolResult { .. } => None,
        })
        .collect()
}

fn map_stop_reason(stop_reason: Option<&str>) -> Option<String> {
    stop_reason.map(|reason| match reason {
        "tool_use" => "tool_use".to_string(),
        "end_turn" => "end_turn".to_string(),
        "max_tokens" => "max_tokens".to_string(),
        _ => reason.to_string(),
    })
}

#[derive(Default)]
struct StreamParseState {
    tool_input_json_by_index: HashMap<u32, String>,
}

#[async_trait]
impl Provider for BedrockProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, AiError> {
        let url = self.build_url(false);
        let system = get_system_prompt(&request.messages, request.system.as_deref());

        let body = BedrockRequest {
            model_id: self.model_id.clone(),
            messages: convert_messages(&request.messages),
            system,
            inference_config: BedrockInferenceConfig {
                max_tokens: request.max_tokens,
            },
            tool_config: convert_tools(request.tools.as_deref()),
        };

        let payload = serde_json::to_string(&body)
            .map_err(|error| AiError::ParseError(error.to_string()))?;

        let headers = HashMap::new();
        let signed_headers = self.sign_request("POST", &url, &headers, &payload)?;

        let mut req = self
            .client
            .post(&url)
            .header("Content-Type", "application/json");

        for (key, value) in signed_headers {
            req = req.header(key, value);
        }

        let response = req.body(payload).send().await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AiError::ApiError {
                status: status.as_u16(),
                message: error_text,
            });
        }

        let api_response: BedrockResponse = response
            .json()
            .await
            .map_err(|error| AiError::ParseError(error.to_string()))?;

        Ok(CompletionResponse {
            content: convert_response_content(api_response.output.message.content),
            input_tokens: api_response
                .usage
                .as_ref()
                .and_then(|usage| usage.input_tokens),
            output_tokens: api_response
                .usage
                .as_ref()
                .and_then(|usage| usage.output_tokens),
            model: self.model_id.clone(),
            stop_reason: map_stop_reason(api_response.stop_reason.as_deref()),
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
        tx: mpsc::Sender<Result<StreamEvent, AiError>>,
    ) -> Result<(), AiError> {
        let url = self.build_url(true);
        let system = get_system_prompt(&request.messages, request.system.as_deref());

        let body = BedrockRequest {
            model_id: self.model_id.clone(),
            messages: convert_messages(&request.messages),
            system,
            inference_config: BedrockInferenceConfig {
                max_tokens: request.max_tokens,
            },
            tool_config: convert_tools(request.tools.as_deref()),
        };

        let payload = serde_json::to_string(&body)
            .map_err(|error| AiError::ParseError(error.to_string()))?;

        let headers = HashMap::new();
        let signed_headers = self.sign_request("POST", &url, &headers, &payload)?;

        let mut req = self
            .client
            .post(&url)
            .header("Content-Type", "application/json");

        for (key, value) in signed_headers {
            req = req.header(key, value);
        }

        let response = req.body(payload).send().await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AiError::ApiError {
                status: status.as_u16(),
                message: error_text,
            });
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut state = StreamParseState::default();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(AiError::RequestFailed)?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if line.trim().is_empty() {
                    continue;
                }

                match serde_json::from_str::<BedrockStreamEvent>(&line) {
                    Ok(event) => {
                        if let Some(stream_event) = process_bedrock_event(event, &mut state) {
                            if tx.send(Ok(stream_event)).await.is_err() {
                                return Ok(());
                            }
                        }
                    }
                    Err(_) => {}
                }
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "bedrock"
    }
}

fn process_bedrock_event(
    event: BedrockStreamEvent,
    state: &mut StreamParseState,
) -> Option<StreamEvent> {
    match event {
        BedrockStreamEvent::MessageStart => Some(StreamEvent::MessageStart),
        BedrockStreamEvent::ContentBlockStart {
            content_block_index,
            start,
        } => match start {
            BedrockContentBlockStart::ToolUse { tool_use } => {
                state
                    .tool_input_json_by_index
                    .insert(content_block_index, String::new());

                Some(StreamEvent::ContentBlockStart {
                    index: content_block_index,
                    content_block: ContentBlock::ToolUse {
                        id: tool_use.tool_use_id,
                        name: tool_use.name,
                        input: tool_use.input,
                    },
                })
            }
        },
        BedrockStreamEvent::ContentBlockDelta {
            content_block_index,
            delta,
        } => match delta {
            BedrockDelta::Text { text } => Some(StreamEvent::ContentBlockDelta {
                index: content_block_index,
                delta: ContentDelta::TextDelta { text },
            }),
            BedrockDelta::ToolUse { tool_use } => {
                state
                    .tool_input_json_by_index
                    .entry(content_block_index)
                    .or_default()
                    .push_str(&tool_use.input);

                Some(StreamEvent::ContentBlockDelta {
                    index: content_block_index,
                    delta: ContentDelta::InputJsonDelta {
                        partial_json: tool_use.input,
                    },
                })
            }
        },
        BedrockStreamEvent::ContentBlockStop {
            content_block_index,
        } => {
            state.tool_input_json_by_index.remove(&content_block_index);
            Some(StreamEvent::ContentBlockStop {
                index: content_block_index,
            })
        }
        BedrockStreamEvent::MessageStop { stop_reason } => Some(StreamEvent::MessageDelta {
            stop_reason: map_stop_reason(stop_reason.as_deref()),
            usage: None,
        }),
        BedrockStreamEvent::Metadata { usage } => Some(StreamEvent::MessageDelta {
            stop_reason: None,
            usage: usage.map(|u| Usage {
                input_tokens: u.input_tokens,
                output_tokens: u.output_tokens,
            }),
        }),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_bedrock_request_serialization() {
        let request = BedrockRequest {
            model_id: "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
            messages: vec![BedrockMessage {
                role: "user".to_string(),
                content: vec![BedrockContentBlock::Text {
                    text: "Hello".to_string(),
                }],
            }],
            system: Some(vec![BedrockSystemBlock {
                text: "You are a helpful assistant.".to_string(),
            }]),
            inference_config: BedrockInferenceConfig { max_tokens: 1024 },
            tool_config: None,
        };

        let value = serde_json::to_value(&request).expect("serialize bedrock request");

        assert_eq!(value["modelId"], "anthropic.claude-3-5-sonnet-20241022-v2:0");
        assert_eq!(value["messages"][0]["role"], "user");
        assert_eq!(value["messages"][0]["content"][0]["text"], "Hello");
        assert_eq!(value["system"][0]["text"], "You are a helpful assistant.");
        assert_eq!(value["inferenceConfig"]["maxTokens"], 1024);
    }

    #[test]
    fn test_bedrock_message_conversion() {
        let messages = vec![Message {
            role: Role::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "tool_123".to_string(),
                content: "result".to_string(),
                is_error: false,
            }],
        }];

        let converted = convert_messages(&messages);

        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].role, "user");
        assert_eq!(converted[0].content.len(), 1);

        match &converted[0].content[0] {
            BedrockContentBlock::ToolResult { tool_result } => {
                assert_eq!(tool_result.tool_use_id, "tool_123");
                assert_eq!(tool_result.content[0].text, "result");
                assert_eq!(tool_result.status.as_deref(), Some("success"));
            }
            _ => panic!("expected tool result"),
        }
    }

    #[test]
    fn test_bedrock_tool_conversion() {
        let tools = vec![ToolDefinition {
            name: "bash".to_string(),
            description: "Execute bash".to_string(),
            input_schema: json!({"type": "object", "properties": {"command": {"type": "string"}}}),
        }];

        let config = convert_tools(Some(&tools)).expect("convert tools");

        assert_eq!(config.tools.len(), 1);
        assert_eq!(config.tools[0].tool_spec.name, "bash");
        assert_eq!(config.tools[0].tool_spec.description, "Execute bash");
    }

    #[test]
    fn test_bedrock_response_parsing() {
        let value = json!({
            "output": {
                "message": {
                    "role": "assistant",
                    "content": [
                        {"text": "Hello!"}
                    ]
                }
            },
            "stopReason": "end_turn",
            "usage": {"inputTokens": 10, "outputTokens": 5}
        });

        let response: BedrockResponse = serde_json::from_value(value).expect("deserialize");
        let blocks = convert_response_content(response.output.message.content);

        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], ContentBlock::Text { text } if text == "Hello!"));
        assert_eq!(map_stop_reason(response.stop_reason.as_deref()).as_deref(), Some("end_turn"));
    }

    #[test]
    fn test_bedrock_stream_event_parsing() {
        let event_json = json!({
            "type": "contentBlockDelta",
            "contentBlockIndex": 0,
            "delta": {"text": "Hello"}
        });

        let event: BedrockStreamEvent = serde_json::from_value(event_json).expect("parse event");
        let mut state = StreamParseState::default();
        let stream_event = process_bedrock_event(event, &mut state).expect("process event");

        match stream_event {
            StreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(index, 0);
                assert!(matches!(delta, ContentDelta::TextDelta { text } if text == "Hello"));
            }
            _ => panic!("expected content block delta"),
        }
    }

    #[test]
    fn test_bedrock_usage_parsing() {
        let event_json = json!({
            "type": "metadata",
            "usage": {"inputTokens": 100, "outputTokens": 50}
        });

        let event: BedrockStreamEvent = serde_json::from_value(event_json).expect("parse event");
        let mut state = StreamParseState::default();
        let stream_event = process_bedrock_event(event, &mut state).expect("process event");

        match stream_event {
            StreamEvent::MessageDelta { usage, .. } => {
                assert_eq!(usage.as_ref().unwrap().input_tokens, Some(100));
                assert_eq!(usage.as_ref().unwrap().output_tokens, Some(50));
            }
            _ => panic!("expected message delta"),
        }
    }

    #[test]
    fn test_bedrock_sigv4_canonical_request() {
        let provider = BedrockProvider::new(
            "us-east-1".to_string(),
            "test_key".to_string(),
            "test_secret".to_string(),
            None,
            "test-model".to_string(),
        );

        let url = provider.build_url(false);
        let headers = HashMap::new();
        let payload = "{}";

        let result = provider.sign_request("POST", &url, &headers, payload);
        assert!(result.is_ok());

        let signed = result.unwrap();
        assert!(signed.contains_key("Authorization"));
        assert!(signed.contains_key("x-amz-date"));
        assert!(signed.contains_key("x-amz-content-sha256"));
        assert!(signed["Authorization"].starts_with("AWS4-HMAC-SHA256"));
    }

    #[test]
    fn test_bedrock_provider_name() {
        let provider = BedrockProvider::new(
            "us-east-1".to_string(),
            "key".to_string(),
            "secret".to_string(),
            None,
            "model".to_string(),
        );

        assert_eq!(provider.name(), "bedrock");
    }
}
