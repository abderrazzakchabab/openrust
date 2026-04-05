use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::sync::mpsc;

use super::types::{
    AiError, CompletionRequest, CompletionResponse, ContentBlock, ContentDelta, Message, Role,
    StreamEvent, ToolDefinition,
};
use super::Provider;

const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";

pub struct OpenAIProvider {
    api_key: String,
    client: Client,
    base_url: String,
    extra_headers: HashMap<String, String>,
}

impl OpenAIProvider {
    pub fn new(
        api_key: String,
        base_url: Option<String>,
        extra_headers: Option<HashMap<String, String>>,
    ) -> Self {
        OpenAIProvider {
            api_key,
            client: Client::new(),
            base_url: base_url.unwrap_or_else(|| OPENAI_API_URL.to_string()),
            extra_headers: extra_headers.unwrap_or_default(),
        }
    }

    /// Standard OpenAI API
    pub fn openai(api_key: String) -> Self {
        Self::new(api_key, None, None)
    }

    /// OpenRouter: OpenAI-compatible with extra headers
    pub fn openrouter(api_key: String) -> Self {
        let mut headers = HashMap::new();
        headers.insert(
            "HTTP-Referer".to_string(),
            "https://openrust.dev".to_string(),
        );
        headers.insert("X-Title".to_string(), "OpenRust".to_string());
        Self::new(
            api_key,
            Some("https://openrouter.ai/api/v1/chat/completions".to_string()),
            Some(headers),
        )
    }

    /// Ollama: local server, no auth
    pub fn ollama(base_url: Option<String>) -> Self {
        Self::new(
            String::new(),
            Some(
                base_url
                    .unwrap_or_else(|| "http://localhost:11434/v1/chat/completions".to_string()),
            ),
            None,
        )
    }

    /// Generic OpenAI-compatible: user provides base_url
    pub fn generic(
        api_key: String,
        base_url: String,
        extra_headers: Option<HashMap<String, String>>,
    ) -> Self {
        Self::new(api_key, Some(base_url), extra_headers)
    }
}

#[derive(Debug, Clone, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAIToolDef>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIToolDef {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIFunctionDef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIFunctionDef {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIToolCall {
    id: String,
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIFunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
    model: String,
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamResponse {
    choices: Vec<OpenAIStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamChoice {
    delta: Option<OpenAIStreamDelta>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamDelta {
    role: Option<String>,
    content: Option<String>,
    tool_calls: Option<Vec<OpenAIStreamToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamToolCallDelta {
    index: u32,
    id: Option<String>,
    #[serde(rename = "type")]
    tool_type: Option<String>,
    function: Option<OpenAIStreamFunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamFunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Default)]
struct OpenAIStreamToolCallState {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
    started: bool,
}

#[derive(Default)]
struct StreamParseState {
    message_started: bool,
    tool_calls: HashMap<u32, OpenAIStreamToolCallState>,
}

enum SseLineParseResult {
    None,
    Events(Vec<StreamEvent>),
    Done,
}

impl From<&ToolDefinition> for OpenAIToolDef {
    fn from(tool: &ToolDefinition) -> Self {
        OpenAIToolDef {
            tool_type: "function".to_string(),
            function: OpenAIFunctionDef {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.input_schema.clone(),
            },
        }
    }
}

fn convert_tools(tools: Option<&[ToolDefinition]>) -> Option<Vec<OpenAIToolDef>> {
    tools.map(|definitions| definitions.iter().map(OpenAIToolDef::from).collect())
}

fn convert_messages(messages: &[Message]) -> Vec<OpenAIMessage> {
    let mut converted = Vec::new();

    for message in messages {
        let mut text_content = String::new();
        let mut tool_calls = Vec::new();
        let mut tool_results = Vec::new();

        for block in &message.content {
            match block {
                ContentBlock::Text { text } => text_content.push_str(text),
                ContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(OpenAIToolCall {
                        id: id.clone(),
                        tool_type: "function".to_string(),
                        function: OpenAIFunctionCall {
                            name: name.clone(),
                            arguments: input.to_string(),
                        },
                    });
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    let result_content = if *is_error {
                        format!("ERROR: {content}")
                    } else {
                        content.clone()
                    };

                    tool_results.push((tool_use_id.clone(), result_content));
                }
                ContentBlock::Thinking { .. } => {}
            }
        }

        let text_content = if text_content.is_empty() {
            None
        } else {
            Some(text_content)
        };

        if !tool_calls.is_empty() {
            converted.push(OpenAIMessage {
                role: Role::Assistant.to_string(),
                content: text_content,
                tool_calls: Some(tool_calls),
                tool_call_id: None,
            });
        } else if let Some(content) = text_content {
            converted.push(OpenAIMessage {
                role: message.role.to_string(),
                content: Some(content),
                tool_calls: None,
                tool_call_id: None,
            });
        }

        for (tool_call_id, content) in tool_results {
            converted.push(OpenAIMessage {
                role: "tool".to_string(),
                content: Some(content),
                tool_calls: None,
                tool_call_id: Some(tool_call_id),
            });
        }
    }

    converted
}

fn parse_tool_arguments(arguments: &str) -> Result<Value, AiError> {
    let trimmed = arguments.trim();

    if trimmed.is_empty() {
        return Ok(Value::Object(Map::new()));
    }

    serde_json::from_str(trimmed).map_err(|error| {
        AiError::ParseError(format!(
            "Failed to parse OpenAI tool call arguments as JSON: {error}"
        ))
    })
}

fn convert_tool_call_to_content_block(tool_call: OpenAIToolCall) -> Result<ContentBlock, AiError> {
    let input = parse_tool_arguments(&tool_call.function.arguments)?;

    Ok(ContentBlock::ToolUse {
        id: tool_call.id,
        name: tool_call.function.name,
        input,
    })
}

fn convert_response_message(message: OpenAIMessage) -> Result<Vec<ContentBlock>, AiError> {
    let mut blocks = Vec::new();

    if let Some(content) = message.content {
        if !content.is_empty() {
            blocks.push(ContentBlock::Text { text: content });
        }
    }

    if let Some(tool_calls) = message.tool_calls {
        for tool_call in tool_calls {
            blocks.push(convert_tool_call_to_content_block(tool_call)?);
        }
    }

    Ok(blocks)
}

fn map_finish_reason(finish_reason: Option<&str>) -> Option<String> {
    finish_reason.map(|reason| match reason {
        "tool_calls" => "tool_use".to_string(),
        "stop" => "end_turn".to_string(),
        _ => reason.to_string(),
    })
}

fn maybe_emit_tool_call_start(
    index: u32,
    tool_call: &mut OpenAIStreamToolCallState,
    events: &mut Vec<StreamEvent>,
) {
    if tool_call.started {
        return;
    }

    if let (Some(id), Some(name)) = (tool_call.id.clone(), tool_call.name.clone()) {
        events.push(StreamEvent::ContentBlockStart {
            index,
            content_block: ContentBlock::ToolUse {
                id,
                name,
                input: Value::Object(Map::new()),
            },
        });

        tool_call.started = true;
    }
}

fn parse_tool_call_delta(
    tool_call_delta: OpenAIStreamToolCallDelta,
    state: &mut StreamParseState,
    events: &mut Vec<StreamEvent>,
) {
    let index = tool_call_delta.index;
    let tool_call = state.tool_calls.entry(index).or_default();

    if let Some(id) = tool_call_delta.id {
        tool_call.id = Some(id);
    }

    if let Some(function) = tool_call_delta.function {
        if let Some(name) = function.name {
            tool_call.name = Some(name);
        }

        maybe_emit_tool_call_start(index, tool_call, events);

        if let Some(arguments) = function.arguments {
            if !arguments.is_empty() {
                tool_call.arguments.push_str(&arguments);
                events.push(StreamEvent::ContentBlockDelta {
                    index,
                    delta: ContentDelta::InputJsonDelta {
                        partial_json: arguments,
                    },
                });
            }
        }
    }

    if let Some(tool_type) = tool_call_delta.tool_type {
        let _ = tool_type;
    }
}

fn parse_stream_delta(
    delta: OpenAIStreamDelta,
    state: &mut StreamParseState,
    events: &mut Vec<StreamEvent>,
) {
    let has_content = delta
        .content
        .as_ref()
        .is_some_and(|content| !content.is_empty())
        || delta
            .tool_calls
            .as_ref()
            .is_some_and(|tool_calls| !tool_calls.is_empty());

    if !state.message_started && (delta.role.as_deref() == Some("assistant") || has_content) {
        events.push(StreamEvent::MessageStart);
        state.message_started = true;
    }

    if let Some(text) = delta.content {
        if !text.is_empty() {
            events.push(StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentDelta::TextDelta { text },
            });
        }
    }

    if let Some(tool_calls) = delta.tool_calls {
        for tool_call in tool_calls {
            parse_tool_call_delta(tool_call, state, events);
        }
    }
}

fn finish_tool_calls(state: &mut StreamParseState, events: &mut Vec<StreamEvent>) {
    let mut indices = state.tool_calls.keys().copied().collect::<Vec<_>>();
    indices.sort_unstable();

    for index in indices {
        if let Some(tool_call) = state.tool_calls.get_mut(&index) {
            maybe_emit_tool_call_start(index, tool_call, events);

            if !tool_call.arguments.is_empty() {
                let _ = serde_json::from_str::<Value>(&tool_call.arguments);
            }
        }

        events.push(StreamEvent::ContentBlockStop { index });
    }

    state.tool_calls.clear();
}

fn parse_stream_response(
    data: &str,
    state: &mut StreamParseState,
) -> Result<Vec<StreamEvent>, AiError> {
    let response: OpenAIStreamResponse =
        serde_json::from_str(data).map_err(|error| AiError::ParseError(error.to_string()))?;

    let mut events = Vec::new();

    for choice in response.choices {
        if let Some(delta) = choice.delta {
            parse_stream_delta(delta, state, &mut events);
        }

        if let Some(finish_reason) = choice.finish_reason {
            if finish_reason == "tool_calls" {
                finish_tool_calls(state, &mut events);
            }

            events.push(StreamEvent::MessageDelta {
                stop_reason: map_finish_reason(Some(&finish_reason)),
                usage: None,
            });
        }
    }

    Ok(events)
}

fn process_sse_line(
    line: &str,
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
            return Ok(SseLineParseResult::Done);
        }

        let events = parse_stream_response(&data, state)?;
        if events.is_empty() {
            Ok(SseLineParseResult::None)
        } else {
            Ok(SseLineParseResult::Events(events))
        }
    } else if line.starts_with(':') {
        Ok(SseLineParseResult::None)
    } else if let Some(data) = line.strip_prefix("data: ") {
        current_data_lines.push(data.to_string());
        Ok(SseLineParseResult::None)
    } else if let Some(data) = line.strip_prefix("data:") {
        current_data_lines.push(data.to_string());
        Ok(SseLineParseResult::None)
    } else {
        Ok(SseLineParseResult::None)
    }
}

#[async_trait]
impl Provider for OpenAIProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, AiError> {
        let mut messages = convert_messages(&request.messages);

        if let Some(system) = &request.system {
            messages.insert(
                0,
                OpenAIMessage {
                    role: "system".to_string(),
                    content: Some(system.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                },
            );
        }

        let body = OpenAIRequest {
            model: request.model.clone(),
            messages,
            max_tokens: request.max_tokens,
            stream: false,
            tools: convert_tools(request.tools.as_deref()),
        };

        let mut http_request = self.client.post(&self.base_url);

        if !self.api_key.is_empty() {
            http_request = http_request.bearer_auth(&self.api_key);
        }

        for (key, value) in &self.extra_headers {
            http_request = http_request.header(key, value);
        }

        let response = http_request.json(&body).send().await?;

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
            .map_err(|error| AiError::ParseError(error.to_string()))?;

        let mut content = Vec::new();
        let mut stop_reason = None;

        for choice in api_response.choices {
            if stop_reason.is_none() {
                stop_reason = map_finish_reason(choice.finish_reason.as_deref());
            }

            content.extend(convert_response_message(choice.message)?);
        }

        Ok(CompletionResponse {
            content,
            input_tokens: api_response
                .usage
                .as_ref()
                .and_then(|usage| usage.prompt_tokens),
            output_tokens: api_response
                .usage
                .as_ref()
                .and_then(|usage| usage.completion_tokens),
            model: api_response.model,
            stop_reason,
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
        tx: mpsc::Sender<Result<StreamEvent, AiError>>,
    ) -> Result<(), AiError> {
        let mut messages = convert_messages(&request.messages);

        if let Some(system) = &request.system {
            messages.insert(
                0,
                OpenAIMessage {
                    role: "system".to_string(),
                    content: Some(system.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                },
            );
        }

        let body = OpenAIRequest {
            model: request.model.clone(),
            messages,
            max_tokens: request.max_tokens,
            stream: true,
            tools: convert_tools(request.tools.as_deref()),
        };

        let mut http_request = self.client.post(&self.base_url);

        if !self.api_key.is_empty() {
            http_request = http_request.bearer_auth(&self.api_key);
        }

        for (key, value) in &self.extra_headers {
            http_request = http_request.header(key, value);
        }

        let response = http_request.json(&body).send().await?;

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
        let mut current_data_lines = Vec::new();
        let mut state = StreamParseState::default();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(AiError::RequestFailed)?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                match process_sse_line(&line, &mut current_data_lines, &mut state)? {
                    SseLineParseResult::None => {}
                    SseLineParseResult::Done => {
                        let _ = tx.send(Ok(StreamEvent::MessageStop)).await;
                        return Ok(());
                    }
                    SseLineParseResult::Events(events) => {
                        for event in events {
                            if tx.send(Ok(event)).await.is_err() {
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }

        if !buffer.is_empty() {
            match process_sse_line(&buffer, &mut current_data_lines, &mut state)? {
                SseLineParseResult::None => {}
                SseLineParseResult::Done => {
                    let _ = tx.send(Ok(StreamEvent::MessageStop)).await;
                    return Ok(());
                }
                SseLineParseResult::Events(events) => {
                    for event in events {
                        if tx.send(Ok(event)).await.is_err() {
                            return Ok(());
                        }
                    }
                }
            }
        }

        match process_sse_line("", &mut current_data_lines, &mut state)? {
            SseLineParseResult::None => {}
            SseLineParseResult::Done => {
                let _ = tx.send(Ok(StreamEvent::MessageStop)).await;
                return Ok(());
            }
            SseLineParseResult::Events(events) => {
                for event in events {
                    if tx.send(Ok(event)).await.is_err() {
                        return Ok(());
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_openai_request_serializes_tools() {
        let tools = vec![ToolDefinition {
            name: "bash".to_string(),
            description: "Execute a bash command".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string" }
                },
                "required": ["command"]
            }),
        }];

        let request = OpenAIRequest {
            model: "gpt-4o".to_string(),
            messages: vec![OpenAIMessage {
                role: "user".to_string(),
                content: Some("list files".to_string()),
                tool_calls: None,
                tool_call_id: None,
            }],
            max_tokens: 512,
            stream: false,
            tools: convert_tools(Some(tools.as_slice())),
        };

        let value = serde_json::to_value(&request).expect("serialize openai request");

        assert_eq!(value["tools"][0]["type"], "function");
        assert_eq!(value["tools"][0]["function"]["name"], "bash");
        assert_eq!(
            value["tools"][0]["function"]["description"],
            "Execute a bash command"
        );
        assert_eq!(
            value["tools"][0]["function"]["parameters"]["type"],
            "object"
        );
        assert!(value["tools"][0]["function"].get("input_schema").is_none());
    }

    #[test]
    fn test_openai_response_parses_tool_calls() {
        let value = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc123",
                        "type": "function",
                        "function": {
                            "name": "bash",
                            "arguments": "{\"command\": \"ls -la\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "model": "gpt-4o",
            "usage": {"prompt_tokens": 25, "completion_tokens": 42}
        });

        let response: OpenAIResponse = serde_json::from_value(value).expect("deserialize response");
        let choice = &response.choices[0];
        let blocks = convert_response_message(choice.message.clone()).expect("convert response");

        assert_eq!(choice.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(
            map_finish_reason(choice.finish_reason.as_deref()).as_deref(),
            Some("tool_use")
        );
        assert_eq!(blocks.len(), 1);

        match &blocks[0] {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "call_abc123");
                assert_eq!(name, "bash");
                assert_eq!(input["command"], "ls -la");
            }
            other => panic!("expected tool_use block, got {other:?}"),
        }
    }

    #[test]
    fn test_openai_message_conversion() {
        let messages = vec![
            Message {
                role: Role::Assistant,
                content: vec![
                    ContentBlock::Text {
                        text: "I'll run that.".to_string(),
                    },
                    ContentBlock::ToolUse {
                        id: "call_abc123".to_string(),
                        name: "bash".to_string(),
                        input: json!({"command": "ls -la"}),
                    },
                ],
            },
            Message {
                role: Role::User,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: "call_abc123".to_string(),
                    content: "file1.txt\nfile2.txt".to_string(),
                    is_error: false,
                }],
            },
        ];

        let converted = convert_messages(&messages);

        assert_eq!(converted.len(), 2);
        assert_eq!(converted[0].role, "assistant");
        assert_eq!(converted[0].content.as_deref(), Some("I'll run that."));
        assert!(converted[0].tool_calls.is_some());

        let tool_calls = converted[0]
            .tool_calls
            .as_ref()
            .expect("assistant tool calls");
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_abc123");
        assert_eq!(tool_calls[0].function.name, "bash");
        assert_eq!(tool_calls[0].function.arguments, r#"{"command":"ls -la"}"#);

        assert_eq!(converted[1].role, "tool");
        assert_eq!(converted[1].tool_call_id.as_deref(), Some("call_abc123"));
        assert_eq!(
            converted[1].content.as_deref(),
            Some("file1.txt\nfile2.txt")
        );
    }

    #[test]
    fn test_openai_stream_tool_calls() {
        let mut current_data_lines = Vec::new();
        let mut state = StreamParseState::default();
        let mut events = Vec::new();
        let mut done = false;

        let lines = vec![
            format!(
                "data: {}",
                json!({"choices": [{"delta": {"role": "assistant"}}]})
            ),
            "".to_string(),
            format!(
                "data: {}",
                json!({
                    "choices": [{
                        "delta": {
                            "tool_calls": [{
                                "index": 0,
                                "id": "call_abc123",
                                "type": "function",
                                "function": {
                                    "name": "bash",
                                    "arguments": ""
                                }
                            }]
                        }
                    }]
                })
            ),
            "".to_string(),
            format!(
                "data: {}",
                json!({
                    "choices": [{
                        "delta": {
                            "tool_calls": [{
                                "index": 0,
                                "function": {
                                    "arguments": r#"{"com"#
                                }
                            }]
                        }
                    }]
                })
            ),
            "".to_string(),
            format!(
                "data: {}",
                json!({
                    "choices": [{
                        "delta": {
                            "tool_calls": [{
                                "index": 0,
                                "function": {
                                    "arguments": r#"mand": "ls -la"}"#
                                }
                            }]
                        }
                    }]
                })
            ),
            "".to_string(),
            format!(
                "data: {}",
                json!({
                    "choices": [{
                        "delta": {},
                        "finish_reason": "tool_calls"
                    }]
                })
            ),
            "".to_string(),
            "data: [DONE]".to_string(),
            "".to_string(),
        ];

        for line in lines {
            match process_sse_line(&line, &mut current_data_lines, &mut state)
                .expect("parse sse line")
            {
                SseLineParseResult::None => {}
                SseLineParseResult::Done => {
                    done = true;
                    break;
                }
                SseLineParseResult::Events(parsed_events) => events.extend(parsed_events),
            }
        }

        assert!(done);
        assert!(state.tool_calls.is_empty());
        assert!(matches!(events[0], StreamEvent::MessageStart));
        assert!(matches!(
            events[1],
            StreamEvent::ContentBlockStart {
                index: 0,
                content_block: ContentBlock::ToolUse { .. }
            }
        ));
        assert!(matches!(
            events[2],
            StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentDelta::InputJsonDelta { .. }
            }
        ));
        assert!(matches!(
            events[3],
            StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentDelta::InputJsonDelta { .. }
            }
        ));
        assert!(matches!(
            events[4],
            StreamEvent::ContentBlockStop { index: 0 }
        ));

        match &events[5] {
            StreamEvent::MessageDelta { stop_reason, usage } => {
                assert_eq!(stop_reason.as_deref(), Some("tool_use"));
                assert!(usage.is_none());
            }
            other => panic!("expected message_delta, got {other:?}"),
        }
    }

    #[test]
    fn test_openai_default_base_url() {
        let provider = OpenAIProvider::new("test-key".to_string(), None, None);
        assert_eq!(provider.base_url, OPENAI_API_URL);
        assert_eq!(provider.api_key, "test-key");
        assert!(provider.extra_headers.is_empty());
    }

    #[test]
    fn test_openai_custom_base_url() {
        let custom_url = "https://custom.openai.com/v1/chat/completions";
        let provider =
            OpenAIProvider::new("test-key".to_string(), Some(custom_url.to_string()), None);
        assert_eq!(provider.base_url, custom_url);
        assert_eq!(provider.api_key, "test-key");
        assert!(provider.extra_headers.is_empty());
    }

    #[test]
    fn test_openrouter_constructor() {
        let provider = OpenAIProvider::openrouter("test-key".to_string());
        assert_eq!(
            provider.base_url,
            "https://openrouter.ai/api/v1/chat/completions"
        );
        assert_eq!(provider.api_key, "test-key");
        assert_eq!(provider.extra_headers.len(), 2);
        assert_eq!(
            provider.extra_headers.get("HTTP-Referer"),
            Some(&"https://openrust.dev".to_string())
        );
        assert_eq!(
            provider.extra_headers.get("X-Title"),
            Some(&"OpenRust".to_string())
        );
    }

    #[test]
    fn test_ollama_constructor() {
        let provider = OpenAIProvider::ollama(None);
        assert_eq!(
            provider.base_url,
            "http://localhost:11434/v1/chat/completions"
        );
        assert!(provider.api_key.is_empty());
        assert!(provider.extra_headers.is_empty());
    }

    #[test]
    fn test_ollama_custom_base_url() {
        let custom_url = "http://my-ollama:8080/v1/chat/completions";
        let provider = OpenAIProvider::ollama(Some(custom_url.to_string()));
        assert_eq!(provider.base_url, custom_url);
        assert!(provider.api_key.is_empty());
        assert!(provider.extra_headers.is_empty());
    }

    #[test]
    fn test_generic_constructor() {
        let mut headers = HashMap::new();
        headers.insert("X-Custom-Header".to_string(), "custom-value".to_string());

        let provider = OpenAIProvider::generic(
            "test-key".to_string(),
            "https://custom.api.com/v1/chat/completions".to_string(),
            Some(headers.clone()),
        );

        assert_eq!(
            provider.base_url,
            "https://custom.api.com/v1/chat/completions"
        );
        assert_eq!(provider.api_key, "test-key");
        assert_eq!(provider.extra_headers, headers);
    }

    #[test]
    fn test_provider_extra_headers_stored() {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer custom".to_string());
        headers.insert("X-API-Key".to_string(), "secret".to_string());

        let provider = OpenAIProvider::new("test-key".to_string(), None, Some(headers.clone()));

        assert_eq!(provider.extra_headers.len(), 2);
        assert_eq!(
            provider.extra_headers.get("Authorization"),
            Some(&"Bearer custom".to_string())
        );
        assert_eq!(
            provider.extra_headers.get("X-API-Key"),
            Some(&"secret".to_string())
        );
    }

    #[test]
    fn test_openai_convenience_constructor() {
        let provider = OpenAIProvider::openai("test-key".to_string());
        assert_eq!(provider.base_url, OPENAI_API_URL);
        assert_eq!(provider.api_key, "test-key");
        assert!(provider.extra_headers.is_empty());
    }
}
