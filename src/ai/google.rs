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

const GEMINI_API_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

pub struct GeminiProvider {
    api_key: String,
    client: Client,
    base_url: String,
}

impl GeminiProvider {
    pub fn new(api_key: String) -> Self {
        GeminiProvider {
            api_key,
            client: Client::new(),
            base_url: GEMINI_API_BASE_URL.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GeminiTool>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "generationConfig")]
    generation_config: Option<GeminiGenerationConfig>,
}

#[derive(Debug, Clone, Serialize)]
struct GeminiGenerationConfig {
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum GeminiPart {
    Text {
        text: String,
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: GeminiFunctionCall,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: GeminiFunctionResponse,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiFunctionCall {
    name: String,
    args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiFunctionResponse {
    name: String,
    response: Value,
}

#[derive(Debug, Clone, Serialize)]
struct GeminiTool {
    #[serde(rename = "functionDeclarations")]
    function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Debug, Clone, Serialize)]
struct GeminiFunctionDeclaration {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<GeminiUsageMetadata>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct GeminiUsageMetadata {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: Option<u32>,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: Option<u32>,
}

#[derive(Default)]
struct StreamParseState {
    message_started: bool,
    current_text_index: u32,
    tool_calls: HashMap<u32, StreamToolCallState>,
    next_tool_index: u32,
}

#[derive(Default)]
struct StreamToolCallState {
    id: String,
    name: String,
    args_json: String,
    started: bool,
}

enum SseLineParseResult {
    None,
    Events(Vec<StreamEvent>),
    Done,
}

fn convert_messages(messages: &[Message], system: Option<&str>) -> Vec<GeminiContent> {
    let mut contents = Vec::new();

    if let Some(system_text) = system {
        contents.push(GeminiContent {
            role: "user".to_string(),
            parts: vec![GeminiPart::Text {
                text: system_text.to_string(),
            }],
        });
    }

    for message in messages {
        if message.role == Role::System {
            continue;
        }

        let role = match message.role {
            Role::User => "user",
            Role::Assistant => "model",
            Role::System => continue,
        };

        let mut parts = Vec::new();

        for block in &message.content {
            match block {
                ContentBlock::Text { text } => {
                    parts.push(GeminiPart::Text { text: text.clone() });
                }
                ContentBlock::ToolUse { id, name, input } => {
                    parts.push(GeminiPart::FunctionCall {
                        function_call: GeminiFunctionCall {
                            name: name.clone(),
                            args: input.clone(),
                        },
                    });
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    let response_value = if *is_error {
                        serde_json::json!({
                            "error": content
                        })
                    } else {
                        serde_json::from_str::<Value>(content).unwrap_or_else(|_| {
                            serde_json::json!({
                                "result": content
                            })
                        })
                    };

                    parts.push(GeminiPart::FunctionResponse {
                        function_response: GeminiFunctionResponse {
                            name: tool_use_id.clone(),
                            response: response_value,
                        },
                    });
                }
                ContentBlock::Thinking { .. } => {}
            }
        }

        if !parts.is_empty() {
            let has_function_response = parts
                .iter()
                .any(|p| matches!(p, GeminiPart::FunctionResponse { .. }));

            if has_function_response {
                let mut text_parts = Vec::new();
                let mut function_parts = Vec::new();

                for part in parts {
                    match part {
                        GeminiPart::FunctionResponse { .. } => function_parts.push(part),
                        _ => text_parts.push(part),
                    }
                }

                if !text_parts.is_empty() {
                    contents.push(GeminiContent {
                        role: role.to_string(),
                        parts: text_parts,
                    });
                }

                if !function_parts.is_empty() {
                    contents.push(GeminiContent {
                        role: "function".to_string(),
                        parts: function_parts,
                    });
                }
            } else {
                contents.push(GeminiContent {
                    role: role.to_string(),
                    parts,
                });
            }
        }
    }

    contents
}

fn convert_tools(tools: Option<&[ToolDefinition]>) -> Option<Vec<GeminiTool>> {
    tools.map(|definitions| {
        vec![GeminiTool {
            function_declarations: definitions
                .iter()
                .map(|tool| {
                    let parameters = convert_schema_types(tool.input_schema.clone());

                    GeminiFunctionDeclaration {
                        name: tool.name.clone(),
                        description: tool.description.clone(),
                        parameters,
                    }
                })
                .collect(),
        }]
    })
}

fn convert_schema_types(schema: Value) -> Value {
    match schema {
        Value::Object(mut map) => {
            if let Some(type_value) = map.get("type") {
                if let Some(type_str) = type_value.as_str() {
                    map.insert(
                        "type".to_string(),
                        Value::String(type_str.to_uppercase()),
                    );
                }
            }

            if let Some(props) = map.get("properties") {
                if let Value::Object(props_map) = props {
                    let converted_props: serde_json::Map<String, Value> = props_map
                        .iter()
                        .map(|(k, v)| (k.clone(), convert_schema_types(v.clone())))
                        .collect();
                    map.insert("properties".to_string(), Value::Object(converted_props));
                }
            }

            if let Some(items) = map.get("items") {
                map.insert("items".to_string(), convert_schema_types(items.clone()));
            }

            Value::Object(map)
        }
        _ => schema,
    }
}

fn convert_response_content(content: GeminiContent) -> Vec<ContentBlock> {
    let mut blocks = Vec::new();

    for part in content.parts {
        match part {
            GeminiPart::Text { text } => {
                blocks.push(ContentBlock::Text { text });
            }
            GeminiPart::FunctionCall { function_call } => {
                blocks.push(ContentBlock::ToolUse {
                    id: format!("gemini_{}", function_call.name),
                    name: function_call.name,
                    input: function_call.args,
                });
            }
            GeminiPart::FunctionResponse { .. } => {}
        }
    }

    blocks
}

fn map_finish_reason(finish_reason: Option<String>) -> Option<String> {
    finish_reason.map(|reason| match reason.as_str() {
        "STOP" => "end_turn".to_string(),
        _ => reason.to_lowercase(),
    })
}

fn parse_stream_chunk(
    data: &str,
    state: &mut StreamParseState,
) -> Result<Vec<StreamEvent>, AiError> {
    let response: GeminiResponse =
        serde_json::from_str(data).map_err(|error| AiError::ParseError(error.to_string()))?;

    let mut events = Vec::new();

    for candidate in response.candidates {
        if !state.message_started {
            events.push(StreamEvent::MessageStart);
            state.message_started = true;
        }

        for part in candidate.content.parts {
            match part {
                GeminiPart::Text { text } => {
                    if !text.is_empty() {
                        events.push(StreamEvent::ContentBlockDelta {
                            index: state.current_text_index,
                            delta: ContentDelta::TextDelta { text },
                        });
                    }
                }
                GeminiPart::FunctionCall { function_call } => {
                    let index = state.next_tool_index;
                    state.next_tool_index += 1;

                    let tool_state = state.tool_calls.entry(index).or_insert_with(|| {
                        StreamToolCallState {
                            id: format!("gemini_{}", function_call.name),
                            name: function_call.name.clone(),
                            args_json: String::new(),
                            started: false,
                        }
                    });

                    if !tool_state.started {
                        events.push(StreamEvent::ContentBlockStart {
                            index,
                            content_block: ContentBlock::ToolUse {
                                id: tool_state.id.clone(),
                                name: function_call.name.clone(),
                                input: Value::Object(serde_json::Map::new()),
                            },
                        });
                        tool_state.started = true;
                    }

                    let args_str = function_call.args.to_string();
                    if !args_str.is_empty() && args_str != "{}" {
                        tool_state.args_json.push_str(&args_str);
                        events.push(StreamEvent::ContentBlockDelta {
                            index,
                            delta: ContentDelta::InputJsonDelta {
                                partial_json: args_str,
                            },
                        });
                    }
                }
                GeminiPart::FunctionResponse { .. } => {}
            }
        }

        if let Some(finish_reason) = candidate.finish_reason {
            for (index, _) in state.tool_calls.iter() {
                events.push(StreamEvent::ContentBlockStop { index: *index });
            }
            state.tool_calls.clear();

            events.push(StreamEvent::MessageDelta {
                stop_reason: map_finish_reason(Some(finish_reason)),
                usage: response.usage_metadata.as_ref().map(|usage| Usage {
                    input_tokens: usage.prompt_token_count,
                    output_tokens: usage.candidates_token_count,
                }),
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

        let events = parse_stream_chunk(&data, state)?;
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
impl Provider for GeminiProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, AiError> {
        let contents = convert_messages(&request.messages, request.system.as_deref());
        let body = GeminiRequest {
            contents,
            tools: convert_tools(request.tools.as_deref()),
            generation_config: Some(GeminiGenerationConfig {
                max_output_tokens: request.max_tokens,
            }),
        };

        let url = format!(
            "{}/models/{}:generateContent?key={}",
            self.base_url, request.model, self.api_key
        );

        let response = self
            .client
            .post(&url)
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

        let api_response: GeminiResponse = response
            .json()
            .await
            .map_err(|error| AiError::ParseError(error.to_string()))?;

        let mut content = Vec::new();
        let mut stop_reason = None;

        for candidate in api_response.candidates {
            if stop_reason.is_none() {
                stop_reason = map_finish_reason(candidate.finish_reason);
            }
            content.extend(convert_response_content(candidate.content));
        }

        Ok(CompletionResponse {
            content,
            input_tokens: api_response
                .usage_metadata
                .as_ref()
                .and_then(|usage| usage.prompt_token_count),
            output_tokens: api_response
                .usage_metadata
                .as_ref()
                .and_then(|usage| usage.candidates_token_count),
            model: request.model,
            stop_reason,
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
        tx: mpsc::Sender<Result<StreamEvent, AiError>>,
    ) -> Result<(), AiError> {
        let contents = convert_messages(&request.messages, request.system.as_deref());
        let body = GeminiRequest {
            contents,
            tools: convert_tools(request.tools.as_deref()),
            generation_config: Some(GeminiGenerationConfig {
                max_output_tokens: request.max_tokens,
            }),
        };

        let url = format!(
            "{}/models/{}:streamGenerateContent?alt=sse&key={}",
            self.base_url, request.model, self.api_key
        );

        let response = self
            .client
            .post(&url)
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

        let _ = tx.send(Ok(StreamEvent::MessageStop)).await;
        Ok(())
    }

    fn name(&self) -> &str {
        "google"
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_gemini_provider_name() {
        let provider = GeminiProvider::new("test_key".to_string());
        assert_eq!(provider.name(), "google");
    }

    #[test]
    fn test_gemini_request_serialization() {
        let request = GeminiRequest {
            contents: vec![GeminiContent {
                role: "user".to_string(),
                parts: vec![GeminiPart::Text {
                    text: "Hello".to_string(),
                }],
            }],
            tools: None,
            generation_config: Some(GeminiGenerationConfig {
                max_output_tokens: 1024,
            }),
        };

        let value = serde_json::to_value(&request).expect("serialize gemini request");
        assert_eq!(value["contents"][0]["role"], "user");
        assert_eq!(value["contents"][0]["parts"][0]["text"], "Hello");
        assert_eq!(value["generationConfig"]["maxOutputTokens"], 1024);
    }

    #[test]
    fn test_gemini_message_conversion_user() {
        let messages = vec![Message::new_user("Hello, Gemini!")];
        let contents = convert_messages(&messages, None);

        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].role, "user");
        assert_eq!(contents[0].parts.len(), 1);

        match &contents[0].parts[0] {
            GeminiPart::Text { text } => assert_eq!(text, "Hello, Gemini!"),
            _ => panic!("expected text part"),
        }
    }

    #[test]
    fn test_gemini_message_conversion_assistant() {
        let messages = vec![Message::new_assistant("Hello back!")];
        let contents = convert_messages(&messages, None);

        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].role, "model");
    }

    #[test]
    fn test_gemini_message_conversion_with_system() {
        let messages = vec![Message::new_user("Hello")];
        let contents = convert_messages(&messages, Some("You are a helpful assistant"));

        assert_eq!(contents.len(), 2);
        assert_eq!(contents[0].role, "user");
        match &contents[0].parts[0] {
            GeminiPart::Text { text } => assert_eq!(text, "You are a helpful assistant"),
            _ => panic!("expected text part"),
        }
    }

    #[test]
    fn test_gemini_message_conversion_tool_use() {
        let messages = vec![Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "tool_1".to_string(),
                name: "bash".to_string(),
                input: json!({"command": "ls -la"}),
            }],
        }];

        let contents = convert_messages(&messages, None);
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].role, "model");

        match &contents[0].parts[0] {
            GeminiPart::FunctionCall { function_call } => {
                assert_eq!(function_call.name, "bash");
                assert_eq!(function_call.args["command"], "ls -la");
            }
            _ => panic!("expected function call part"),
        }
    }

    #[test]
    fn test_gemini_message_conversion_tool_result() {
        let messages = vec![Message {
            role: Role::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "tool_1".to_string(),
                content: "file1.txt\nfile2.txt".to_string(),
                is_error: false,
            }],
        }];

        let contents = convert_messages(&messages, None);
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].role, "function");

        match &contents[0].parts[0] {
            GeminiPart::FunctionResponse { function_response } => {
                assert_eq!(function_response.name, "tool_1");
            }
            _ => panic!("expected function response part"),
        }
    }

    #[test]
    fn test_gemini_tool_conversion() {
        let tools = vec![ToolDefinition {
            name: "bash".to_string(),
            description: "Execute a bash command".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The command to execute"
                    }
                },
                "required": ["command"]
            }),
        }];

        let gemini_tools = convert_tools(Some(&tools)).expect("convert tools");
        assert_eq!(gemini_tools.len(), 1);
        assert_eq!(gemini_tools[0].function_declarations.len(), 1);

        let func = &gemini_tools[0].function_declarations[0];
        assert_eq!(func.name, "bash");
        assert_eq!(func.description, "Execute a bash command");
        assert_eq!(func.parameters["type"], "OBJECT");
        assert_eq!(func.parameters["properties"]["command"]["type"], "STRING");
    }

    #[test]
    fn test_gemini_response_parsing() {
        let value = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{
                        "text": "Hello! How can I help you?"
                    }]
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 20
            }
        });

        let response: GeminiResponse =
            serde_json::from_value(value).expect("deserialize gemini response");
        assert_eq!(response.candidates.len(), 1);

        let blocks = convert_response_content(response.candidates[0].content.clone());
        assert_eq!(blocks.len(), 1);

        match &blocks[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Hello! How can I help you?"),
            _ => panic!("expected text block"),
        }
    }

    #[test]
    fn test_gemini_usage_parsing() {
        let usage = GeminiUsageMetadata {
            prompt_token_count: Some(100),
            candidates_token_count: Some(50),
        };

        let our_usage = Usage {
            input_tokens: usage.prompt_token_count,
            output_tokens: usage.candidates_token_count,
        };

        assert_eq!(our_usage.input_tokens, Some(100));
        assert_eq!(our_usage.output_tokens, Some(50));
    }

    #[test]
    fn test_gemini_finish_reason_mapping() {
        assert_eq!(
            map_finish_reason(Some("STOP".to_string())),
            Some("end_turn".to_string())
        );
        assert_eq!(map_finish_reason(None), None);
    }

    #[test]
    fn test_gemini_function_call_parsing() {
        let value = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{
                        "functionCall": {
                            "name": "find_theaters",
                            "args": {
                                "movie": "Barbie",
                                "location": "Mountain View, CA"
                            }
                        }
                    }]
                },
                "finishReason": "STOP"
            }]
        });

        let response: GeminiResponse =
            serde_json::from_value(value).expect("deserialize gemini response");
        let blocks = convert_response_content(response.candidates[0].content.clone());

        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(name, "find_theaters");
                assert_eq!(input["movie"], "Barbie");
                assert_eq!(input["location"], "Mountain View, CA");
                assert!(id.starts_with("gemini_"));
            }
            _ => panic!("expected tool use block"),
        }
    }

    #[test]
    fn test_gemini_function_response_conversion() {
        let messages = vec![Message {
            role: Role::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "tool_123".to_string(),
                content: r#"{"result": "success"}"#.to_string(),
                is_error: false,
            }],
        }];

        let contents = convert_messages(&messages, None);
        assert_eq!(contents[0].role, "function");

        match &contents[0].parts[0] {
            GeminiPart::FunctionResponse { function_response } => {
                assert_eq!(function_response.name, "tool_123");
                assert_eq!(function_response.response["result"], "success");
            }
            _ => panic!("expected function response"),
        }
    }

    #[test]
    fn test_gemini_stream_response_parsing() {
        let data = r#"{
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{
                        "text": "Hello"
                    }]
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 5,
                "candidatesTokenCount": 2
            }
        }"#;

        let mut state = StreamParseState::default();
        let events = parse_stream_chunk(data, &mut state).expect("parse stream chunk");

        assert!(!events.is_empty());
        assert!(matches!(events[0], StreamEvent::MessageStart));
        assert!(matches!(
            events[1],
            StreamEvent::ContentBlockDelta {
                delta: ContentDelta::TextDelta { .. },
                ..
            }
        ));
    }
}
