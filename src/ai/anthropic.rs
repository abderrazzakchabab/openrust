use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;

use super::types::{
    AiError, CompletionRequest, CompletionResponse, ContentBlock, ContentDelta, Message, Role,
    StreamEvent, ToolDefinition, Usage,
};
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

#[derive(Debug, Clone, Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolDefinition>>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: AnthropicMessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum AnthropicMessageContent {
    Text(String),
    Blocks(Vec<AnthropicMessageBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicMessageBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        is_error: bool,
    },
    Thinking {
        thinking: String,
    },
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
    model: String,
    stop_reason: Option<String>,
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicContent {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
struct AnthropicUsage {
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ContentBlockStartEvent {
    index: u32,
    content_block: AnthropicStreamContentBlock,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicStreamContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
}

#[derive(Debug, Deserialize)]
struct ContentBlockDeltaEvent {
    index: u32,
    delta: AnthropicStreamDelta,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicStreamDelta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Deserialize)]
struct ContentBlockStopEvent {
    index: u32,
}

#[derive(Debug, Deserialize)]
struct MessageDeltaEvent {
    delta: MessageDeltaPayload,
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
struct MessageDeltaPayload {
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamErrorEvent {
    error: StreamErrorPayload,
}

#[derive(Debug, Deserialize)]
struct StreamErrorPayload {
    message: String,
}

#[derive(Default)]
struct StreamParseState {
    tool_input_json_by_index: HashMap<u32, String>,
}

enum SseLineParseResult {
    None,
    Event(StreamEvent),
    Done,
}

fn convert_messages(messages: &[Message]) -> Vec<AnthropicMessage> {
    messages
        .iter()
        .filter(|message| message.role != Role::System)
        .map(|message| AnthropicMessage {
            role: message.role.to_string(),
            content: convert_message_content(&message.content),
        })
        .collect()
}

fn convert_message_content(content: &[ContentBlock]) -> AnthropicMessageContent {
    if content
        .iter()
        .all(|block| matches!(block, ContentBlock::Text { .. }))
    {
        let text = content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>();

        AnthropicMessageContent::Text(text)
    } else {
        AnthropicMessageContent::Blocks(content.iter().map(convert_content_block).collect())
    }
}

fn convert_content_block(block: &ContentBlock) -> AnthropicMessageBlock {
    match block {
        ContentBlock::Text { text } => AnthropicMessageBlock::Text { text: text.clone() },
        ContentBlock::ToolUse { id, name, input } => AnthropicMessageBlock::ToolUse {
            id: id.clone(),
            name: name.clone(),
            input: input.clone(),
        },
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => AnthropicMessageBlock::ToolResult {
            tool_use_id: tool_use_id.clone(),
            content: content.clone(),
            is_error: *is_error,
        },
        ContentBlock::Thinking { thinking } => AnthropicMessageBlock::Thinking {
            thinking: thinking.clone(),
        },
    }
}

fn get_system_prompt(messages: &[Message]) -> Option<String> {
    messages
        .iter()
        .find(|message| message.role == Role::System)
        .map(Message::text_content)
}

fn convert_response_content(content: Vec<AnthropicContent>) -> Vec<ContentBlock> {
    content
        .into_iter()
        .filter_map(|block| match block {
            AnthropicContent::Text { text } => Some(ContentBlock::Text { text }),
            AnthropicContent::ToolUse { id, name, input } => {
                Some(ContentBlock::ToolUse { id, name, input })
            }
            AnthropicContent::Unknown => None,
        })
        .collect()
}

fn parse_usage(usage: Option<AnthropicUsage>) -> Option<Usage> {
    usage.map(|usage| Usage {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
    })
}

fn parse_stream_event(
    event_type: &str,
    data: &str,
    state: &mut StreamParseState,
) -> Result<Option<StreamEvent>, AiError> {
    match event_type {
        "message_start" => Ok(Some(StreamEvent::MessageStart)),
        "content_block_start" => {
            let event: ContentBlockStartEvent = serde_json::from_str(data)
                .map_err(|error| AiError::ParseError(error.to_string()))?;

            let content_block = match event.content_block {
                AnthropicStreamContentBlock::Text { text } => ContentBlock::Text { text },
                AnthropicStreamContentBlock::ToolUse { id, name, input } => {
                    let initial_json = match &input {
                        Value::Object(map) if map.is_empty() => String::new(),
                        _ => input.to_string(),
                    };

                    state
                        .tool_input_json_by_index
                        .insert(event.index, initial_json);

                    ContentBlock::ToolUse { id, name, input }
                }
            };

            Ok(Some(StreamEvent::ContentBlockStart {
                index: event.index,
                content_block,
            }))
        }
        "content_block_delta" => {
            let event: ContentBlockDeltaEvent = serde_json::from_str(data)
                .map_err(|error| AiError::ParseError(error.to_string()))?;

            let delta = match event.delta {
                AnthropicStreamDelta::TextDelta { text } => ContentDelta::TextDelta { text },
                AnthropicStreamDelta::InputJsonDelta { partial_json } => {
                    state
                        .tool_input_json_by_index
                        .entry(event.index)
                        .or_default()
                        .push_str(&partial_json);

                    ContentDelta::InputJsonDelta { partial_json }
                }
            };

            Ok(Some(StreamEvent::ContentBlockDelta {
                index: event.index,
                delta,
            }))
        }
        "content_block_stop" => {
            let event: ContentBlockStopEvent = serde_json::from_str(data)
                .map_err(|error| AiError::ParseError(error.to_string()))?;

            if let Some(json_input) = state.tool_input_json_by_index.remove(&event.index) {
                if !json_input.is_empty() {
                    let _ = serde_json::from_str::<Value>(&json_input);
                }
            }

            Ok(Some(StreamEvent::ContentBlockStop { index: event.index }))
        }
        "message_delta" => {
            let event: MessageDeltaEvent = serde_json::from_str(data)
                .map_err(|error| AiError::ParseError(error.to_string()))?;

            Ok(Some(StreamEvent::MessageDelta {
                stop_reason: event.delta.stop_reason,
                usage: parse_usage(event.usage),
            }))
        }
        "message_stop" => Ok(Some(StreamEvent::MessageStop)),
        "error" => {
            let message = serde_json::from_str::<StreamErrorEvent>(data)
                .map(|event| event.error.message)
                .unwrap_or_else(|_| "Anthropic stream error".to_string());

            Ok(Some(StreamEvent::Error { message }))
        }
        _ => Ok(None),
    }
}

fn process_sse_line(
    line: &str,
    current_event_type: &mut Option<String>,
    current_data_lines: &mut Vec<String>,
    state: &mut StreamParseState,
) -> Result<SseLineParseResult, AiError> {
    let line = line.trim_end_matches('\r');

    if line.is_empty() {
        if current_data_lines.is_empty() {
            return Ok(SseLineParseResult::None);
        }

        let data = current_data_lines.join("\n");
        current_data_lines.clear();

        if data == "[DONE]" {
            current_event_type.take();
            return Ok(SseLineParseResult::Done);
        }

        if let Some(event_type) = current_event_type.take() {
            if let Some(event) = parse_stream_event(&event_type, &data, state)? {
                return Ok(SseLineParseResult::Event(event));
            }
        }

        return Ok(SseLineParseResult::None);
    }

    if line.starts_with(':') {
        return Ok(SseLineParseResult::None);
    }

    if let Some(event_type) = line.strip_prefix("event:") {
        *current_event_type = Some(event_type.trim().to_string());
        return Ok(SseLineParseResult::None);
    }

    if let Some(data) = line.strip_prefix("data: ") {
        current_data_lines.push(data.to_string());
        return Ok(SseLineParseResult::None);
    }

    if let Some(data) = line.strip_prefix("data:") {
        current_data_lines.push(data.to_string());
        return Ok(SseLineParseResult::None);
    }

    Ok(SseLineParseResult::None)
}

#[async_trait]
impl Provider for AnthropicProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, AiError> {
        let system = request
            .system
            .or_else(|| get_system_prompt(&request.messages));
        let body = AnthropicRequest {
            model: request.model.clone(),
            messages: convert_messages(&request.messages),
            max_tokens: request.max_tokens,
            system,
            tools: request.tools.clone(),
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
            .map_err(|error| AiError::ParseError(error.to_string()))?;

        Ok(CompletionResponse {
            content: convert_response_content(api_response.content),
            input_tokens: api_response
                .usage
                .as_ref()
                .and_then(|usage| usage.input_tokens),
            output_tokens: api_response
                .usage
                .as_ref()
                .and_then(|usage| usage.output_tokens),
            model: api_response.model,
            stop_reason: api_response.stop_reason,
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
        tx: mpsc::Sender<Result<StreamEvent, AiError>>,
    ) -> Result<(), AiError> {
        let system = request
            .system
            .or_else(|| get_system_prompt(&request.messages));
        let body = AnthropicRequest {
            model: request.model.clone(),
            messages: convert_messages(&request.messages),
            max_tokens: request.max_tokens,
            system,
            tools: request.tools.clone(),
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
        let mut current_event_type = None;
        let mut current_data_lines = Vec::new();
        let mut state = StreamParseState::default();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(AiError::RequestFailed)?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                match process_sse_line(
                    &line,
                    &mut current_event_type,
                    &mut current_data_lines,
                    &mut state,
                )? {
                    SseLineParseResult::None => {}
                    SseLineParseResult::Done => return Ok(()),
                    SseLineParseResult::Event(event) => {
                        if tx.send(Ok(event)).await.is_err() {
                            return Ok(());
                        }
                    }
                }
            }
        }

        if !buffer.is_empty() {
            match process_sse_line(
                &buffer,
                &mut current_event_type,
                &mut current_data_lines,
                &mut state,
            )? {
                SseLineParseResult::None => {}
                SseLineParseResult::Done => return Ok(()),
                SseLineParseResult::Event(event) => {
                    if tx.send(Ok(event)).await.is_err() {
                        return Ok(());
                    }
                }
            }
        }

        match process_sse_line(
            "",
            &mut current_event_type,
            &mut current_data_lines,
            &mut state,
        )? {
            SseLineParseResult::None | SseLineParseResult::Done => {}
            SseLineParseResult::Event(event) => {
                let _ = tx.send(Ok(event)).await;
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "anthropic"
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_anthropic_request_serializes_tools() {
        let request = AnthropicRequest {
            model: "claude-sonnet-4-20250514".to_string(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: AnthropicMessageContent::Text("List files".to_string()),
            }],
            max_tokens: 1024,
            system: None,
            tools: Some(vec![ToolDefinition {
                name: "bash".to_string(),
                description: "Execute a bash command".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "command": { "type": "string" }
                    },
                    "required": ["command"]
                }),
            }]),
            stream: false,
        };

        let value = serde_json::to_value(&request).expect("serialize anthropic request");

        assert!(value.get("tools").is_some());
        assert_eq!(value["tools"][0]["name"], "bash");
        assert_eq!(value["tools"][0]["description"], "Execute a bash command");
        assert_eq!(value["tools"][0]["input_schema"]["type"], "object");
    }

    #[test]
    fn test_anthropic_response_parses_tool_use() {
        let value = json!({
            "content": [
                {"type": "text", "text": "I'll list the files for you."},
                {
                    "type": "tool_use",
                    "id": "toolu_01A",
                    "name": "bash",
                    "input": {"command": "ls -la"}
                }
            ],
            "model": "claude-sonnet-4-20250514",
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 25, "output_tokens": 42}
        });

        let response: AnthropicResponse =
            serde_json::from_value(value).expect("deserialize anthropic response");
        let blocks = convert_response_content(response.content);

        assert_eq!(response.stop_reason.as_deref(), Some("tool_use"));
        assert_eq!(blocks.len(), 2);
        assert_eq!(
            blocks[0],
            ContentBlock::Text {
                text: "I'll list the files for you.".to_string()
            }
        );

        match &blocks[1] {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "toolu_01A");
                assert_eq!(name, "bash");
                assert_eq!(input["command"], "ls -la");
            }
            other => panic!("expected tool_use block, got {other:?}"),
        }
    }

    #[test]
    fn test_anthropic_message_conversion() {
        let messages = vec![Message {
            role: Role::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "toolu_01A".to_string(),
                content: "file1.txt\nfile2.txt".to_string(),
                is_error: false,
            }],
        }];

        let converted = convert_messages(&messages);
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].role, "user");

        match &converted[0].content {
            AnthropicMessageContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    AnthropicMessageBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        assert_eq!(tool_use_id, "toolu_01A");
                        assert_eq!(content, "file1.txt\nfile2.txt");
                        assert!(!is_error);
                    }
                    other => panic!("expected tool_result block, got {other:?}"),
                }
            }
            other => panic!("expected block-array content, got {other:?}"),
        }

        let text_only = vec![Message::new_user("hello")];
        let text_converted = convert_messages(&text_only);

        match &text_converted[0].content {
            AnthropicMessageContent::Text(text) => assert_eq!(text, "hello"),
            other => panic!("expected string content, got {other:?}"),
        }
    }

    #[test]
    fn test_stream_event_parsing() {
        let mut current_event_type = None;
        let mut current_data_lines = Vec::new();
        let mut state = StreamParseState::default();
        let mut events = Vec::new();

        let lines = vec![
            "event: message_start".to_string(),
            format!(
                "data: {}",
                json!({
                    "type": "message_start",
                    "message": {
                        "model": "claude-sonnet-4-20250514"
                    }
                })
            ),
            "".to_string(),
            "event: content_block_start".to_string(),
            format!(
                "data: {}",
                json!({
                    "type": "content_block_start",
                    "index": 0,
                    "content_block": {"type": "text", "text": ""}
                })
            ),
            "".to_string(),
            "event: content_block_delta".to_string(),
            format!(
                "data: {}",
                json!({
                    "type": "content_block_delta",
                    "index": 0,
                    "delta": {"type": "text_delta", "text": "I'll "}
                })
            ),
            "".to_string(),
            "event: content_block_stop".to_string(),
            format!(
                "data: {}",
                json!({"type": "content_block_stop", "index": 0})
            ),
            "".to_string(),
            "event: content_block_start".to_string(),
            format!(
                "data: {}",
                json!({
                    "type": "content_block_start",
                    "index": 1,
                    "content_block": {
                        "type": "tool_use",
                        "id": "toolu_01A",
                        "name": "bash",
                        "input": {}
                    }
                })
            ),
            "".to_string(),
            "event: content_block_delta".to_string(),
            format!(
                "data: {}",
                json!({
                    "type": "content_block_delta",
                    "index": 1,
                    "delta": {
                        "type": "input_json_delta",
                        "partial_json": r#"{"com"#
                    }
                })
            ),
            "".to_string(),
            "event: content_block_delta".to_string(),
            format!(
                "data: {}",
                json!({
                    "type": "content_block_delta",
                    "index": 1,
                    "delta": {
                        "type": "input_json_delta",
                        "partial_json": r#"mand": "ls"}"#
                    }
                })
            ),
            "".to_string(),
            "event: content_block_stop".to_string(),
            format!(
                "data: {}",
                json!({"type": "content_block_stop", "index": 1})
            ),
            "".to_string(),
            "event: message_delta".to_string(),
            format!(
                "data: {}",
                json!({
                    "type": "message_delta",
                    "delta": {"stop_reason": "tool_use"},
                    "usage": {"output_tokens": 42}
                })
            ),
            "".to_string(),
            "event: message_stop".to_string(),
            format!("data: {}", json!({"type": "message_stop"})),
            "".to_string(),
        ];

        for line in lines {
            match process_sse_line(
                &line,
                &mut current_event_type,
                &mut current_data_lines,
                &mut state,
            )
            .expect("parse sse line")
            {
                SseLineParseResult::None => {}
                SseLineParseResult::Done => break,
                SseLineParseResult::Event(event) => events.push(event),
            }
        }

        assert!(state.tool_input_json_by_index.is_empty());
        assert!(matches!(events[0], StreamEvent::MessageStart));
        assert!(matches!(
            events[1],
            StreamEvent::ContentBlockStart {
                index: 0,
                content_block: ContentBlock::Text { .. }
            }
        ));
        assert!(matches!(
            events[2],
            StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentDelta::TextDelta { .. }
            }
        ));
        assert!(matches!(
            events[3],
            StreamEvent::ContentBlockStop { index: 0 }
        ));
        assert!(matches!(
            events[4],
            StreamEvent::ContentBlockStart {
                index: 1,
                content_block: ContentBlock::ToolUse { .. }
            }
        ));
        assert!(matches!(
            events[5],
            StreamEvent::ContentBlockDelta {
                index: 1,
                delta: ContentDelta::InputJsonDelta { .. }
            }
        ));
        assert!(matches!(
            events[6],
            StreamEvent::ContentBlockDelta {
                index: 1,
                delta: ContentDelta::InputJsonDelta { .. }
            }
        ));
        assert!(matches!(
            events[7],
            StreamEvent::ContentBlockStop { index: 1 }
        ));

        match &events[8] {
            StreamEvent::MessageDelta { stop_reason, usage } => {
                assert_eq!(stop_reason.as_deref(), Some("tool_use"));
                assert_eq!(
                    usage.as_ref().and_then(|value| value.output_tokens),
                    Some(42)
                );
            }
            other => panic!("expected message_delta, got {other:?}"),
        }

        assert!(matches!(events[9], StreamEvent::MessageStop));
    }
}
