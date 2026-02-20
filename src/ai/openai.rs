use anyhow::Result;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::types::{AiError, CompletionRequest, CompletionResponse, Message};
use super::Provider;

const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";

pub struct OpenAIProvider {
    api_key: String,
    client: Client,
    base_url: String,
}

impl OpenAIProvider {
    pub fn new(api_key: String, base_url: Option<String>) -> Self {
        OpenAIProvider {
            api_key,
            client: Client::new(),
            base_url: base_url.unwrap_or_else(|| OPENAI_API_URL.to_string()),
        }
    }
}

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    max_tokens: u32,
    stream: bool,
}

#[derive(Serialize, Deserialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
    model: String,
    usage: Option<OpenAIUsage>,
}

#[derive(Deserialize)]
struct OpenAIChoice {
    message: Option<OpenAIMessage>,
    delta: Option<OpenAIDelta>,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct OpenAIDelta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct OpenAIUsage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
}

fn convert_messages(messages: &[Message]) -> Vec<OpenAIMessage> {
    messages
        .iter()
        .map(|m| OpenAIMessage {
            role: m.role.to_string(),
            content: m.content.clone(),
        })
        .collect()
}

#[async_trait]
impl Provider for OpenAIProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, AiError> {
        let mut msgs = convert_messages(&request.messages);

        // Prepend system message if provided
        if let Some(system) = &request.system {
            msgs.insert(
                0,
                OpenAIMessage {
                    role: "system".to_string(),
                    content: system.clone(),
                },
            );
        }

        let body = OpenAIRequest {
            model: request.model.clone(),
            messages: msgs,
            max_tokens: request.max_tokens,
            stream: false,
        };

        let response = self
            .client
            .post(&self.base_url)
            .bearer_auth(&self.api_key)
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

        let api_response: OpenAIResponse = response
            .json()
            .await
            .map_err(|e| AiError::ParseError(e.to_string()))?;

        let content = api_response
            .choices
            .into_iter()
            .filter_map(|c| c.message)
            .map(|m| m.content)
            .collect::<Vec<_>>()
            .join("");

        Ok(CompletionResponse {
            content,
            input_tokens: api_response.usage.as_ref().and_then(|u| u.prompt_tokens),
            output_tokens: api_response
                .usage
                .as_ref()
                .and_then(|u| u.completion_tokens),
            model: api_response.model,
            stop_reason: None,
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
        tx: mpsc::Sender<Result<String, AiError>>,
    ) -> Result<(), AiError> {
        let mut msgs = convert_messages(&request.messages);

        if let Some(system) = &request.system {
            msgs.insert(
                0,
                OpenAIMessage {
                    role: "system".to_string(),
                    content: system.clone(),
                },
            );
        }

        let body = OpenAIRequest {
            model: request.model.clone(),
            messages: msgs,
            max_tokens: request.max_tokens,
            stream: true,
        };

        let response = self
            .client
            .post(&self.base_url)
            .bearer_auth(&self.api_key)
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

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if line.starts_with("data: ") {
                    let data = &line["data: ".len()..];
                    if data == "[DONE]" {
                        return Ok(());
                    }

                    match serde_json::from_str::<OpenAIResponse>(data) {
                        Ok(event) => {
                            for choice in event.choices {
                                if let Some(delta) = choice.delta {
                                    if let Some(text) = delta.content {
                                        if tx.send(Ok(text)).await.is_err() {
                                            return Ok(());
                                        }
                                    }
                                }
                            }
                        }
                        Err(_) => {}
                    }
                }
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "openai"
    }
}
