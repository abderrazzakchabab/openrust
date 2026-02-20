use anyhow::Result;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::types::{AiError, CompletionRequest, CompletionResponse, Message, Role};
use super::Provider;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    api_key: String,
    client: Client,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self {
        AnthropicProvider {
            api_key,
            client: Client::new(),
        }
    }
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}

#[derive(Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
    model: String,
    stop_reason: Option<String>,
    usage: Option<AnthropicUsage>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
}

#[derive(Deserialize)]
struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<AnthropicDelta>,
    usage: Option<AnthropicUsage>,
    message: Option<AnthropicStreamMessage>,
}

#[derive(Deserialize)]
struct AnthropicDelta {
    #[serde(rename = "type")]
    delta_type: Option<String>,
    text: Option<String>,
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicStreamMessage {
    model: Option<String>,
}

fn convert_messages(messages: &[Message]) -> Vec<AnthropicMessage> {
    messages
        .iter()
        .filter(|m| m.role != Role::System)
        .map(|m| AnthropicMessage {
            role: m.role.to_string(),
            content: m.content.clone(),
        })
        .collect()
}

fn get_system_prompt(messages: &[Message]) -> Option<String> {
    messages
        .iter()
        .find(|m| m.role == Role::System)
        .map(|m| m.content.clone())
}

#[async_trait]
impl Provider for AnthropicProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, AiError> {
        let system = request.system.or_else(|| get_system_prompt(&request.messages));
        let body = AnthropicRequest {
            model: request.model.clone(),
            messages: convert_messages(&request.messages),
            max_tokens: request.max_tokens,
            system,
            stream: false,
        };

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AiError::ApiError {
                status: status.as_u16(),
                message: error_text,
            });
        }

        let api_response: AnthropicResponse = response
            .json()
            .await
            .map_err(|e| AiError::ParseError(e.to_string()))?;

        let content = api_response
            .content
            .into_iter()
            .filter(|c| c.content_type == "text")
            .filter_map(|c| c.text)
            .collect::<Vec<_>>()
            .join("");

        Ok(CompletionResponse {
            content,
            input_tokens: api_response.usage.as_ref().and_then(|u| u.input_tokens),
            output_tokens: api_response.usage.as_ref().and_then(|u| u.output_tokens),
            model: api_response.model,
            stop_reason: api_response.stop_reason,
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
        tx: mpsc::Sender<Result<String, AiError>>,
    ) -> Result<(), AiError> {
        let system = request.system.or_else(|| get_system_prompt(&request.messages));
        let body = AnthropicRequest {
            model: request.model.clone(),
            messages: convert_messages(&request.messages),
            max_tokens: request.max_tokens,
            system,
            stream: true,
        };

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

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

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(AiError::RequestFailed)?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            // Process SSE events from buffer
            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if line.starts_with("data: ") {
                    let data = &line["data: ".len()..];
                    if data == "[DONE]" {
                        return Ok(());
                    }

                    match serde_json::from_str::<AnthropicStreamEvent>(data) {
                        Ok(event) => {
                            if event.event_type == "content_block_delta" {
                                if let Some(delta) = event.delta {
                                    if let Some(text) = delta.text {
                                        if tx.send(Ok(text)).await.is_err() {
                                            return Ok(());
                                        }
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            // Skip malformed events
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "anthropic"
    }
}
