use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use futures::future::join_all;
use serde_json::{Map, Value};
use tokio::sync::{mpsc, Mutex, oneshot};

use crate::agents::AgentConfig;
use crate::ai::types::{
    AiError, CompletionRequest, ContentBlock, ContentDelta, Message, Role, StreamEvent,
    ToolDefinition,
};
use crate::ai::Provider;
use crate::app::{AppEvent, PermissionResponse};
use crate::mcp::McpManager;
use crate::permissions::{PermissionChecker, PermissionResult};
use crate::tools::traits::{ToolOutput, ToolRegistry};

const MAX_ITERATIONS: usize = 50;

struct DoomLoopDetector {
    recent_calls: VecDeque<(String, String)>,
    max_history: usize,
    repetition_threshold: usize,
}

impl DoomLoopDetector {
    fn new(max_history: usize, threshold: usize) -> Self {
        Self {
            recent_calls: VecDeque::new(),
            max_history,
            repetition_threshold: threshold,
        }
    }

    fn record_call(&mut self, tool_name: &str, input: &Value) {
        let input_hash = format!("{}", input);
        self.recent_calls
            .push_back((tool_name.to_string(), input_hash));

        while self.recent_calls.len() > self.max_history {
            self.recent_calls.pop_front();
        }
    }

    fn is_doom_loop(&self) -> bool {
        if self.recent_calls.len() < self.repetition_threshold {
            return false;
        }

        let last_call = match self.recent_calls.back() {
            Some(call) => call,
            None => return false,
        };

        let mut count = 0;
        for call in self.recent_calls.iter().rev() {
            if call == last_call {
                count += 1;
                if count >= self.repetition_threshold {
                    return true;
                }
            }
        }

        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum BlockRef {
    Text(u32),
    ToolUse(u32),
    Other(u32),
}

#[derive(Debug, Clone)]
struct ToolUseAccumulator {
    id: String,
    name: String,
    input_json: String,
}

#[derive(Debug, Default)]
struct StreamAccumulator {
    text_blocks: HashMap<u32, String>,
    tool_use_blocks: HashMap<u32, ToolUseAccumulator>,
    other_blocks: HashMap<u32, ContentBlock>,
    block_order: Vec<BlockRef>,
    seen_blocks: HashSet<BlockRef>,
    stop_reason: Option<String>,
}

impl StreamAccumulator {
    fn register_block(&mut self, block_ref: BlockRef) {
        if self.seen_blocks.insert(block_ref) {
            self.block_order.push(block_ref);
        }
    }

    fn on_block_start(&mut self, index: u32, content_block: ContentBlock) {
        match content_block {
            ContentBlock::Text { text } => {
                self.register_block(BlockRef::Text(index));
                self.text_blocks.insert(index, text);
            }
            ContentBlock::ToolUse { id, name, input } => {
                self.register_block(BlockRef::ToolUse(index));

                let initial_input_json = match input {
                    Value::Object(ref map) if map.is_empty() => String::new(),
                    _ => input.to_string(),
                };

                if let Some(existing) = self.tool_use_blocks.get_mut(&index) {
                    existing.id = id;
                    existing.name = name;

                    if existing.input_json.is_empty() {
                        existing.input_json = initial_input_json;
                    }
                } else {
                    self.tool_use_blocks.insert(
                        index,
                        ToolUseAccumulator {
                            id,
                            name,
                            input_json: initial_input_json,
                        },
                    );
                }
            }
            other => {
                self.register_block(BlockRef::Other(index));
                self.other_blocks.insert(index, other);
            }
        }
    }

    fn on_block_delta(&mut self, index: u32, delta: ContentDelta) {
        match delta {
            ContentDelta::TextDelta { text } => {
                self.register_block(BlockRef::Text(index));
                self.text_blocks.entry(index).or_default().push_str(&text);
            }
            ContentDelta::InputJsonDelta { partial_json } => {
                self.register_block(BlockRef::ToolUse(index));
                self.tool_use_blocks
                    .entry(index)
                    .or_insert_with(|| ToolUseAccumulator {
                        id: String::new(),
                        name: String::new(),
                        input_json: String::new(),
                    })
                    .input_json
                    .push_str(&partial_json);
            }
        }
    }

    fn into_content_blocks(self) -> Vec<ContentBlock> {
        let mut content_blocks = Vec::new();

        for block_ref in self.block_order {
            match block_ref {
                BlockRef::Text(index) => {
                    if let Some(text) = self.text_blocks.get(&index) {
                        if !text.is_empty() {
                            content_blocks.push(ContentBlock::Text { text: text.clone() });
                        }
                    }
                }
                BlockRef::ToolUse(index) => {
                    if let Some(tool_use) = self.tool_use_blocks.get(&index) {
                        if !tool_use.id.is_empty() && !tool_use.name.is_empty() {
                            content_blocks.push(ContentBlock::ToolUse {
                                id: tool_use.id.clone(),
                                name: tool_use.name.clone(),
                                input: parse_tool_input(&tool_use.input_json),
                            });
                        }
                    }
                }
                BlockRef::Other(index) => {
                    if let Some(other_block) = self.other_blocks.get(&index) {
                        content_blocks.push(other_block.clone());
                    }
                }
            }
        }

        content_blocks
    }
}

#[derive(Debug, Clone)]
struct ToolCall {
    id: String,
    name: String,
    input: Value,
}

fn parse_tool_input(input_json: &str) -> Value {
    let trimmed = input_json.trim();

    if trimmed.is_empty() {
        return Value::Object(Map::new());
    }

    serde_json::from_str(trimmed).unwrap_or_else(|_| Value::Object(Map::new()))
}

fn extract_tool_calls(message: &Message) -> Vec<ToolCall> {
    message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolUse { id, name, input } => Some(ToolCall {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
            }),
            _ => None,
        })
        .collect()
}

async fn execute_tool_call(
    tool_call: ToolCall,
    registry: Arc<ToolRegistry>,
    event_tx: mpsc::Sender<AppEvent>,
    permissions: Arc<Mutex<PermissionChecker>>,
    mcp_manager: Option<Arc<Mutex<McpManager>>>,
) -> ContentBlock {
    let tool_name = tool_call.name.clone();
    let input_json = tool_call.input.to_string();

    let _ = event_tx
        .send(AppEvent::ToolCallStart {
            name: tool_name.clone(),
            input: input_json.clone(),
        })
        .await;

    if tool_name.contains("::") {
        let parts: Vec<&str> = tool_name.splitn(2, "::").collect();
        if parts.len() == 2 {
            let server_name = parts[0];
            let mcp_tool_name = parts[1];

            if let Some(ref mcp) = mcp_manager {
                let mut manager = mcp.lock().await;
                match manager
                    .call_tool(server_name, mcp_tool_name, tool_call.input.clone())
                    .await
                {
                    Ok(result) => {
                        let content = result
                            .content
                            .iter()
                            .filter_map(|c| c.text.as_ref())
                            .cloned()
                            .collect::<Vec<_>>()
                            .join("\n");

                        let output = ToolOutput {
                            content: content.clone(),
                            is_error: result.is_error,
                        };

                        let _ = event_tx
                            .send(AppEvent::ToolCallComplete {
                                name: tool_name.clone(),
                                output: content.clone(),
                                is_error: output.is_error,
                            })
                            .await;

                        return ContentBlock::ToolResult {
                            tool_use_id: tool_call.id,
                            content,
                            is_error: result.is_error,
                        };
                    }
                    Err(e) => {
                        let error_msg = format!("MCP tool error: {}", e);
                        let output = ToolOutput::error(error_msg.clone());

                        let _ = event_tx
                            .send(AppEvent::ToolCallComplete {
                                name: tool_name.clone(),
                                output: error_msg.clone(),
                                is_error: true,
                            })
                            .await;

                        return ContentBlock::ToolResult {
                            tool_use_id: tool_call.id,
                            content: error_msg,
                            is_error: true,
                        };
                    }
                }
            } else {
                let error_msg = format!("MCP not available for tool: {}", tool_name);
                let output = ToolOutput::error(error_msg.clone());

                let _ = event_tx
                    .send(AppEvent::ToolCallComplete {
                        name: tool_name.clone(),
                        output: error_msg.clone(),
                        is_error: true,
                    })
                    .await;

                return ContentBlock::ToolResult {
                    tool_use_id: tool_call.id,
                    content: error_msg,
                    is_error: true,
                };
            }
        }
    }

    let file_tools = ["read", "write", "edit", "patch", "glob", "list"];
    if file_tools.contains(&tool_name.as_str()) {
        if let Some(path) = extract_file_path(&tool_call.input) {
            let path_check = {
                let checker = permissions.lock().await;
                checker.check_path_access(&path)
            };

            if let PermissionResult::Denied { reason } = path_check {
                let output = ToolOutput::error(reason);
                let _ = event_tx
                    .send(AppEvent::ToolCallComplete {
                        name: tool_name,
                        output: output.content.clone(),
                        is_error: output.is_error,
                    })
                    .await;

                return ContentBlock::ToolResult {
                    tool_use_id: tool_call.id,
                    content: output.content,
                    is_error: output.is_error,
                };
            }
        }
    }

    let permission_result = {
        let checker = permissions.lock().await;
        checker.check_tool(&tool_name, &tool_call.input)
    };

    let output = match permission_result {
        PermissionResult::Allowed => match registry.get(&tool_name) {
            Some(tool) => match tool.execute(tool_call.input).await {
                Ok(output) => output,
                Err(error) => {
                    ToolOutput::error(format!("Tool '{}' execution failed: {}", tool_name, error))
                }
            },
            None => ToolOutput::error(format!("Tool '{}' is not registered", tool_name)),
        },
        PermissionResult::Denied { reason } => ToolOutput::error(reason),
        PermissionResult::NeedsApproval {
            tool_name: perm_tool_name,
            description,
        } => {
            let (response_tx, response_rx) = oneshot::channel();
            
            let _ = event_tx
                .send(AppEvent::PermissionRequest {
                    tool_name: perm_tool_name.clone(),
                    description: description.clone(),
                    input_json: input_json.clone(),
                    response_tx,
                })
                .await;

            let response = response_rx.await.unwrap_or(PermissionResponse::Deny);

            match response {
                PermissionResponse::AllowOnce => {
                    match registry.get(&tool_name) {
                        Some(tool) => match tool.execute(tool_call.input).await {
                            Ok(output) => output,
                            Err(error) => ToolOutput::error(format!(
                                "Tool '{}' execution failed: {}",
                                tool_name, error
                            )),
                        },
                        None => ToolOutput::error(format!("Tool '{}' is not registered", tool_name)),
                    }
                }
                PermissionResponse::AllowSession => {
                    {
                        let mut checker = permissions.lock().await;
                        checker.remember_allow(&tool_name, &input_json);
                    }
                    match registry.get(&tool_name) {
                        Some(tool) => match tool.execute(tool_call.input).await {
                            Ok(output) => output,
                            Err(error) => ToolOutput::error(format!(
                                "Tool '{}' execution failed: {}",
                                tool_name, error
                            )),
                        },
                        None => ToolOutput::error(format!("Tool '{}' is not registered", tool_name)),
                    }
                }
                PermissionResponse::Deny => {
                    ToolOutput::error(format!("Permission denied for tool: {}", tool_name))
                }
            }
        }
    };

    let _ = event_tx
        .send(AppEvent::ToolCallComplete {
            name: tool_name,
            output: output.content.clone(),
            is_error: output.is_error,
        })
        .await;

    ContentBlock::ToolResult {
        tool_use_id: tool_call.id,
        content: output.content,
        is_error: output.is_error,
    }
}

fn extract_file_path(input: &Value) -> Option<String> {
    input
        .get("file_path")
        .or_else(|| input.get("path"))
        .or_else(|| input.get("pattern"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

async fn execute_tool_calls(
    tool_calls: Vec<ToolCall>,
    registry: Arc<ToolRegistry>,
    event_tx: mpsc::Sender<AppEvent>,
    permissions: Arc<Mutex<PermissionChecker>>,
    mcp_manager: Option<Arc<Mutex<McpManager>>>,
) -> Vec<ContentBlock> {
    let tool_tasks =
        tool_calls.into_iter().map(|tool_call| {
            let registry = Arc::clone(&registry);
            let event_tx = event_tx.clone();
            let permissions = Arc::clone(&permissions);
            let mcp_manager = mcp_manager.clone();

            async move {
                execute_tool_call(tool_call, registry, event_tx, permissions, mcp_manager).await
            }
        });

    join_all(tool_tasks).await
}

async fn stream_assistant_message(
    provider: Arc<dyn Provider>,
    request: CompletionRequest,
    event_tx: mpsc::Sender<AppEvent>,
) -> Result<StreamAccumulator, String> {
    let (stream_tx, mut stream_rx) = mpsc::channel::<Result<StreamEvent, AiError>>(100);

    let stream_task =
        tokio::spawn(async move { provider.complete_stream(request, stream_tx).await });

    let mut accumulator = StreamAccumulator::default();
    let mut stream_error = None;

    while let Some(chunk) = stream_rx.recv().await {
        match chunk {
            Ok(StreamEvent::ContentBlockStart {
                index,
                content_block,
            }) => {
                if let ContentBlock::ToolUse { name, input, .. } = &content_block {
                    let _ = event_tx
                        .send(AppEvent::ToolCallStart {
                            name: name.clone(),
                            input: input.to_string(),
                        })
                        .await;
                }

                accumulator.on_block_start(index, content_block);
            }
            Ok(StreamEvent::ContentBlockDelta { index, delta }) => {
                if let ContentDelta::TextDelta { text } = &delta {
                    let _ = event_tx.send(AppEvent::StreamChunk(text.clone())).await;
                }

                accumulator.on_block_delta(index, delta);
            }
            Ok(StreamEvent::ContentBlockStop { .. }) => {}
            Ok(StreamEvent::MessageDelta { stop_reason, .. }) => {
                accumulator.stop_reason = stop_reason;
            }
            Ok(StreamEvent::MessageStart | StreamEvent::MessageStop) => {}
            Ok(StreamEvent::Error { message }) => {
                stream_error = Some(message);
                break;
            }
            Err(error) => {
                stream_error = Some(error.to_string());
                break;
            }
        }
    }

    if let Some(error_message) = stream_error {
        stream_task.abort();
        let _ = stream_task.await;
        return Err(error_message);
    }

    match stream_task.await {
        Ok(Ok(())) => Ok(accumulator),
        Ok(Err(error)) => Err(error.to_string()),
        Err(error) => Err(error.to_string()),
    }
}

async fn run_agent_loop_with_limit(
    provider: Box<dyn Provider>,
    mut messages: Vec<Message>,
    model: String,
    max_tokens: u32,
    system: Option<String>,
    tools: Option<Vec<ToolDefinition>>,
    registry: Arc<ToolRegistry>,
    event_tx: mpsc::Sender<AppEvent>,
    max_iterations: usize,
    permissions: Arc<Mutex<PermissionChecker>>,
    mcp_manager: Option<Arc<Mutex<McpManager>>>,
) -> Vec<Message> {
    let provider = Arc::from(provider);
    let mut doom_detector = DoomLoopDetector::new(20, 3);

    for _iteration in 0..max_iterations {
        let request = CompletionRequest {
            messages: messages.clone(),
            model: model.clone(),
            max_tokens,
            system: system.clone(),
            stream: true,
            tools: tools.clone(),
        };

        let iteration_output = match stream_assistant_message(
            Arc::clone(&provider),
            request,
            event_tx.clone(),
        )
        .await
        {
            Ok(output) => output,
            Err(error_message) => {
                let _ = event_tx
                    .send(AppEvent::StreamError(error_message.clone()))
                    .await;
                messages.push(Message::new_assistant(format!("Error: {error_message}")));
                return messages;
            }
        };

        let stop_reason = iteration_output.stop_reason.clone();
        let assistant_message = Message {
            role: Role::Assistant,
            content: iteration_output.into_content_blocks(),
        };

        messages.push(assistant_message.clone());

        let tool_calls = extract_tool_calls(&assistant_message);
        if stop_reason.as_deref() != Some("tool_use") || tool_calls.is_empty() {
            return messages;
        }

        for tool_call in &tool_calls {
            doom_detector.record_call(&tool_call.name, &tool_call.input);
        }

        if doom_detector.is_doom_loop() {
            let doom_message =
                "Doom loop detected: agent is repeating the same action. Breaking loop."
                    .to_string();
            let _ = event_tx
                .send(AppEvent::StreamError(doom_message.clone()))
                .await;
            messages.push(Message::new_assistant(format!("Error: {doom_message}")));
            return messages;
        }

        let tool_results = execute_tool_calls(
            tool_calls,
            Arc::clone(&registry),
            event_tx.clone(),
            Arc::clone(&permissions),
            mcp_manager.clone(),
        )
        .await;

        messages.push(Message {
            role: Role::User,
            content: tool_results,
        });
    }

    let max_iterations_message = "Max iterations reached".to_string();
    let _ = event_tx
        .send(AppEvent::StreamError(max_iterations_message.clone()))
        .await;
    messages.push(Message::new_assistant(format!(
        "Error: {max_iterations_message}"
    )));
    messages
}

pub async fn run_agent_loop(
    provider: Box<dyn Provider>,
    messages: Vec<Message>,
    model: String,
    max_tokens: u32,
    system: Option<String>,
    tools: Option<Vec<ToolDefinition>>,
    registry: Arc<ToolRegistry>,
    event_tx: mpsc::Sender<AppEvent>,
    agent_config: Option<&AgentConfig>,
    permissions: Arc<Mutex<PermissionChecker>>,
    mcp_manager: Option<Arc<Mutex<McpManager>>>,
) -> Vec<Message> {
    let effective_model = agent_config
        .and_then(|c| c.model_override.as_ref())
        .cloned()
        .unwrap_or(model);

    let effective_max_tokens = agent_config
        .and_then(|c| c.max_tokens_override)
        .unwrap_or(max_tokens);

    let effective_system = agent_config.map(|c| c.system_prompt.clone()).or(system);

    let effective_max_iterations = agent_config
        .map(|c| c.max_iterations)
        .unwrap_or(MAX_ITERATIONS);

    let mut effective_tools = match (tools, agent_config.and_then(|c| c.tool_whitelist.as_ref())) {
        (Some(all_tools), Some(whitelist)) => Some(
            all_tools
                .into_iter()
                .filter(|t| whitelist.contains(&t.name))
                .collect(),
        ),
        (tools, _) => tools,
    };

    if let Some(ref mcp) = mcp_manager {
        let manager = mcp.lock().await;
        let mcp_tools = manager.get_all_tool_definitions();
        if !mcp_tools.is_empty() {
            effective_tools = Some(
                effective_tools
                    .unwrap_or_default()
                    .into_iter()
                    .chain(mcp_tools)
                    .collect(),
            );
        }
    }

    run_agent_loop_with_limit(
        provider,
        messages,
        effective_model,
        effective_max_tokens,
        effective_system,
        effective_tools,
        registry,
        event_tx,
        effective_max_iterations,
        permissions,
        mcp_manager,
    )
    .await
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use serde_json::{json, Value};
    use tokio::sync::mpsc;

    use super::{run_agent_loop_with_limit, ToolDefinition};
    use crate::ai::types::{
        AiError, CompletionRequest, CompletionResponse, ContentBlock, ContentDelta, Message, Role,
        StreamEvent,
    };
    use crate::ai::Provider;
    use crate::app::AppEvent;
    use crate::config::PermissionsConfig;
    use crate::permissions::PermissionChecker;
    use crate::tools::traits::{Tool, ToolOutput, ToolRegistry};

    fn create_test_permissions() -> Arc<tokio::sync::Mutex<PermissionChecker>> {
        let config = PermissionsConfig {
            allow: vec!["*".to_string()],
            ask: vec![],
            deny: vec![],
            bash_allow_patterns: vec!["*".to_string()],
            bash_deny_patterns: vec![],
        };
        Arc::new(tokio::sync::Mutex::new(PermissionChecker::new(&config)))
    }

    #[derive(Debug, Clone)]
    struct MockStreamResponse {
        events: Vec<StreamEvent>,
    }

    #[derive(Default)]
    struct MockProviderState {
        responses: Mutex<Vec<MockStreamResponse>>,
        requests: Mutex<Vec<CompletionRequest>>,
    }

    #[derive(Clone, Default)]
    struct MockProvider {
        state: Arc<MockProviderState>,
    }

    impl MockProvider {
        fn new(responses: Vec<MockStreamResponse>) -> Self {
            MockProvider {
                state: Arc::new(MockProviderState {
                    responses: Mutex::new(responses),
                    requests: Mutex::new(Vec::new()),
                }),
            }
        }

        fn recorded_requests(&self) -> Vec<CompletionRequest> {
            self.state.requests.lock().expect("lock requests").clone()
        }
    }

    #[async_trait]
    impl Provider for MockProvider {
        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse, AiError> {
            Err(AiError::StreamError(
                "MockProvider::complete is not implemented".to_string(),
            ))
        }

        async fn complete_stream(
            &self,
            request: CompletionRequest,
            tx: mpsc::Sender<Result<StreamEvent, AiError>>,
        ) -> Result<(), AiError> {
            self.state
                .requests
                .lock()
                .expect("lock requests")
                .push(request);

            let response = {
                let mut responses = self.state.responses.lock().expect("lock responses");

                if responses.is_empty() {
                    return Err(AiError::StreamError(
                        "No mock stream responses left".to_string(),
                    ));
                }

                responses.remove(0)
            };

            for event in response.events {
                if tx.send(Ok(event)).await.is_err() {
                    break;
                }
            }

            Ok(())
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    struct MockTool {
        name: String,
        description: String,
        execute_fn: Box<dyn Fn(Value) -> anyhow::Result<ToolOutput> + Send + Sync>,
    }

    impl MockTool {
        fn new(
            name: &str,
            execute_fn: Box<dyn Fn(Value) -> anyhow::Result<ToolOutput> + Send + Sync>,
        ) -> Self {
            MockTool {
                name: name.to_string(),
                description: "mock tool".to_string(),
                execute_fn,
            }
        }
    }

    #[async_trait]
    impl Tool for MockTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            &self.description
        }

        fn parameters(&self) -> Value {
            json!({
                "type": "object",
                "properties": {
                    "value": {
                        "type": "string"
                    }
                }
            })
        }

        async fn execute(&self, input: Value) -> anyhow::Result<ToolOutput> {
            (self.execute_fn)(input)
        }
    }

    fn text_response(text: &str, stop_reason: &str) -> MockStreamResponse {
        MockStreamResponse {
            events: vec![
                StreamEvent::MessageStart,
                StreamEvent::ContentBlockDelta {
                    index: 0,
                    delta: ContentDelta::TextDelta {
                        text: text.to_string(),
                    },
                },
                StreamEvent::MessageDelta {
                    stop_reason: Some(stop_reason.to_string()),
                    usage: None,
                },
                StreamEvent::MessageStop,
            ],
        }
    }

    fn tool_use_response(tool_use_id: &str, name: &str, input: Value) -> MockStreamResponse {
        MockStreamResponse {
            events: vec![
                StreamEvent::MessageStart,
                StreamEvent::ContentBlockStart {
                    index: 0,
                    content_block: ContentBlock::ToolUse {
                        id: tool_use_id.to_string(),
                        name: name.to_string(),
                        input: json!({}),
                    },
                },
                StreamEvent::ContentBlockDelta {
                    index: 0,
                    delta: ContentDelta::InputJsonDelta {
                        partial_json: input.to_string(),
                    },
                },
                StreamEvent::ContentBlockStop { index: 0 },
                StreamEvent::MessageDelta {
                    stop_reason: Some("tool_use".to_string()),
                    usage: None,
                },
                StreamEvent::MessageStop,
            ],
        }
    }

    fn multi_tool_use_response(tools: Vec<(&str, &str, Value)>) -> MockStreamResponse {
        let mut events = vec![StreamEvent::MessageStart];

        for (index, (tool_use_id, name, input)) in tools.into_iter().enumerate() {
            let index = index as u32;
            events.push(StreamEvent::ContentBlockStart {
                index,
                content_block: ContentBlock::ToolUse {
                    id: tool_use_id.to_string(),
                    name: name.to_string(),
                    input: json!({}),
                },
            });
            events.push(StreamEvent::ContentBlockDelta {
                index,
                delta: ContentDelta::InputJsonDelta {
                    partial_json: input.to_string(),
                },
            });
            events.push(StreamEvent::ContentBlockStop { index });
        }

        events.push(StreamEvent::MessageDelta {
            stop_reason: Some("tool_use".to_string()),
            usage: None,
        });
        events.push(StreamEvent::MessageStop);

        MockStreamResponse { events }
    }

    fn find_tool_result(messages: &[Message]) -> Option<(String, bool)> {
        messages.iter().find_map(|message| {
            message.content.iter().find_map(|block| match block {
                ContentBlock::ToolResult {
                    content, is_error, ..
                } => Some((content.clone(), *is_error)),
                _ => None,
            })
        })
    }

    #[tokio::test]
    async fn test_agent_loop_no_tools() {
        let provider = MockProvider::new(vec![text_response("hello", "end_turn")]);
        let (event_tx, _event_rx) = mpsc::channel(100);

        let messages = vec![Message::new_user("hi")];
        let final_messages = run_agent_loop_with_limit(
            Box::new(provider),
            messages,
            "mock-model".to_string(),
            1024,
            None,
            None,
            Arc::new(ToolRegistry::new()),
            event_tx,
            5,
            create_test_permissions(),
            None,
        )
        .await;

        assert_eq!(final_messages.len(), 2);
        assert_eq!(final_messages[0].role, Role::User);
        assert_eq!(final_messages[1].role, Role::Assistant);
        assert_eq!(final_messages[1].text_content(), "hello");
    }

    #[tokio::test]
    async fn test_agent_loop_tool_use_cycle() {
        let provider = MockProvider::new(vec![
            tool_use_response("toolu_1", "mock_tool", json!({ "value": "abc" })),
            text_response("done", "end_turn"),
        ]);
        let provider_inspect = provider.clone();

        let tool_inputs = Arc::new(Mutex::new(Vec::new()));
        let tool_inputs_clone = Arc::clone(&tool_inputs);

        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new(
            "mock_tool",
            Box::new(move |input| {
                tool_inputs_clone
                    .lock()
                    .expect("lock tool inputs")
                    .push(input);
                Ok(ToolOutput::success("tool output"))
            }),
        )));

        let (event_tx, _event_rx) = mpsc::channel(100);
        let final_messages = run_agent_loop_with_limit(
            Box::new(provider),
            vec![Message::new_user("run tool")],
            "mock-model".to_string(),
            1024,
            None,
            Some(vec![ToolDefinition {
                name: "mock_tool".to_string(),
                description: "mock".to_string(),
                input_schema: json!({"type": "object"}),
            }]),
            Arc::new(registry),
            event_tx,
            5,
            create_test_permissions(),
            None,
        )
        .await;

        let requests = provider_inspect.recorded_requests();
        assert_eq!(requests.len(), 2);

        let second_request = &requests[1];
        assert!(second_request.messages.iter().any(|message| {
            message.content.iter().any(|block| {
                matches!(
                    block,
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error: false
                    } if tool_use_id == "toolu_1" && content == "tool output"
                )
            })
        }));

        let recorded_inputs = tool_inputs.lock().expect("lock tool inputs").clone();
        assert_eq!(recorded_inputs, vec![json!({ "value": "abc" })]);

        assert_eq!(final_messages.len(), 4);
        assert_eq!(final_messages[1].role, Role::Assistant);
        assert_eq!(final_messages[3].text_content(), "done");
    }

    #[tokio::test]
    async fn test_agent_loop_multi_tool_chain() {
        let provider = MockProvider::new(vec![
            multi_tool_use_response(vec![
                ("toolu_1", "tool_a", json!({"value": "a"})),
                ("toolu_2", "tool_b", json!({"value": "b"})),
            ]),
            text_response("multi done", "end_turn"),
        ]);
        let provider_inspect = provider.clone();

        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new(
            "tool_a",
            Box::new(|_input| Ok(ToolOutput::success("result_a"))),
        )));
        registry.register(Box::new(MockTool::new(
            "tool_b",
            Box::new(|_input| Ok(ToolOutput::success("result_b"))),
        )));

        let (event_tx, _event_rx) = mpsc::channel(100);
        let final_messages = run_agent_loop_with_limit(
            Box::new(provider),
            vec![Message::new_user("run two tools")],
            "mock-model".to_string(),
            1024,
            None,
            None,
            Arc::new(registry),
            event_tx,
            5,
            create_test_permissions(),
            None,
        )
        .await;

        let requests = provider_inspect.recorded_requests();
        assert_eq!(requests.len(), 2);

        let second_request = &requests[1];
        let mut saw_tool_a = false;
        let mut saw_tool_b = false;
        for message in &second_request.messages {
            for block in &message.content {
                if let ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } = block
                {
                    if tool_use_id == "toolu_1" && content == "result_a" && !is_error {
                        saw_tool_a = true;
                    }
                    if tool_use_id == "toolu_2" && content == "result_b" && !is_error {
                        saw_tool_b = true;
                    }
                }
            }
        }

        assert!(saw_tool_a);
        assert!(saw_tool_b);
        assert_eq!(final_messages.last().expect("has final message").text_content(), "multi done");
    }

    #[tokio::test]
    async fn test_agent_loop_max_iterations() {
        let provider = MockProvider::new(vec![
            tool_use_response("toolu_1", "loop_tool", json!({"value": "1"})),
            tool_use_response("toolu_2", "loop_tool", json!({"value": "2"})),
            tool_use_response("toolu_3", "loop_tool", json!({"value": "3"})),
        ]);
        let provider_inspect = provider.clone();

        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new(
            "loop_tool",
            Box::new(|_input| Ok(ToolOutput::success("ok"))),
        )));

        let (event_tx, mut event_rx) = mpsc::channel(100);
        let _final_messages = run_agent_loop_with_limit(
            Box::new(provider),
            vec![Message::new_user("loop")],
            "mock-model".to_string(),
            1024,
            None,
            None,
            Arc::new(registry),
            event_tx,
            3,
            create_test_permissions(),
            None,
        )
        .await;

        assert_eq!(provider_inspect.recorded_requests().len(), 3);

        let mut saw_max_iteration_error = false;
        while let Ok(event) = event_rx.try_recv() {
            if let AppEvent::StreamError(message) = event {
                if message == "Max iterations reached" {
                    saw_max_iteration_error = true;
                }
            }
        }

        assert!(saw_max_iteration_error);
    }

    #[tokio::test]
    async fn test_agent_loop_tool_error() {
        let provider = MockProvider::new(vec![
            tool_use_response("toolu_err", "failing_tool", json!({"value": "bad"})),
            text_response("recovered", "end_turn"),
        ]);

        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new(
            "failing_tool",
            Box::new(|_input| anyhow::bail!("tool exploded")),
        )));

        let (event_tx, _event_rx) = mpsc::channel(100);
        let final_messages = run_agent_loop_with_limit(
            Box::new(provider),
            vec![Message::new_user("fail")],
            "mock-model".to_string(),
            1024,
            None,
            Some(vec![ToolDefinition {
                name: "failing_tool".to_string(),
                description: "mock".to_string(),
                input_schema: json!({"type": "object"}),
            }]),
            Arc::new(registry),
            event_tx,
            5,
            create_test_permissions(),
            None,
        )
        .await;

        let tool_result = find_tool_result(&final_messages).expect("tool result exists");
        assert!(tool_result.1);
        assert!(tool_result.0.contains("tool exploded"));
        assert_eq!(
            final_messages.last().expect("last message").text_content(),
            "recovered"
        );
    }

    #[test]
    fn test_doom_loop_detection() {
        let mut detector = super::DoomLoopDetector::new(20, 3);

        let input = json!({"value": "test"});
        detector.record_call("same_tool", &input);
        assert!(!detector.is_doom_loop());

        detector.record_call("same_tool", &input);
        assert!(!detector.is_doom_loop());

        detector.record_call("same_tool", &input);
        assert!(detector.is_doom_loop());
    }

    #[test]
    fn test_no_doom_loop_varied_calls() {
        let mut detector = super::DoomLoopDetector::new(20, 3);

        detector.record_call("tool1", &json!({"value": "a"}));
        detector.record_call("tool2", &json!({"value": "b"}));
        detector.record_call("tool3", &json!({"value": "c"}));
        detector.record_call("tool1", &json!({"value": "d"}));

        assert!(!detector.is_doom_loop());
    }

    #[test]
    fn test_doom_loop_history_limit() {
        let mut detector = super::DoomLoopDetector::new(5, 3);

        for i in 0..10 {
            detector.record_call("tool", &json!({"value": i}));
        }

        assert_eq!(detector.recent_calls.len(), 5);

        detector.record_call("same", &json!({"x": 1}));
        detector.record_call("same", &json!({"x": 1}));
        detector.record_call("same", &json!({"x": 1}));

        assert!(detector.is_doom_loop());
    }

    #[test]
    fn test_doom_loop_threshold() {
        let mut detector = super::DoomLoopDetector::new(20, 3);

        let input = json!({"value": "test"});
        detector.record_call("tool", &input);
        detector.record_call("tool", &input);

        assert!(!detector.is_doom_loop());
    }

    #[test]
    fn test_mcp_tool_routing_with_namespace() {
        let tool_name = "weather::get_forecast";
        let parts: Vec<&str> = tool_name.splitn(2, "::").collect();

        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "weather");
        assert_eq!(parts[1], "get_forecast");
    }

    #[test]
    fn test_mcp_tool_routing_without_namespace() {
        let tool_name = "read";
        let contains_namespace = tool_name.contains("::");

        assert!(!contains_namespace);
    }

    #[test]
    fn test_mcp_tool_name_splitting() {
        let test_cases = vec![
            ("server::tool", ("server", "tool")),
            ("my-server::do-thing", ("my-server", "do-thing")),
            ("mcp::nested::tool", ("mcp", "nested::tool")),
        ];

        for (input, (expected_server, expected_tool)) in test_cases {
            let parts: Vec<&str> = input.splitn(2, "::").collect();
            assert_eq!(parts.len(), 2);
            assert_eq!(parts[0], expected_server);
            assert_eq!(parts[1], expected_tool);
        }
    }

    #[tokio::test]
    async fn test_agent_loop_mcp_parameter_accepted() {
        let provider = MockProvider::new(vec![text_response("ok", "end_turn")]);

        let (event_tx, _event_rx) = mpsc::channel(100);

        let final_messages = run_agent_loop_with_limit(
            Box::new(provider),
            vec![Message::new_user("test")],
            "mock-model".to_string(),
            1024,
            None,
            None,
            Arc::new(ToolRegistry::new()),
            event_tx,
            5,
            create_test_permissions(),
            None,
        )
        .await;

        assert_eq!(final_messages.len(), 2);
        assert_eq!(final_messages[0].role, Role::User);
        assert_eq!(final_messages[1].role, Role::Assistant);
    }
}
