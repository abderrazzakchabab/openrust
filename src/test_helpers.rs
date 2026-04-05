use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::ai::types::{
    AiError, CompletionRequest, CompletionResponse, ContentBlock, ContentDelta, StreamEvent,
};
use crate::ai::Provider;
use crate::tools::traits::{Tool, ToolOutput};

#[derive(Debug, Clone)]
pub struct MockStreamResponse {
    pub events: Vec<StreamEvent>,
}

#[derive(Default)]
struct MockProviderState {
    responses: Mutex<Vec<MockStreamResponse>>,
    requests: Mutex<Vec<CompletionRequest>>,
}

#[derive(Clone, Default)]
pub struct MockProvider {
    state: Arc<MockProviderState>,
}

impl MockProvider {
    pub fn new(responses: Vec<MockStreamResponse>) -> Self {
        Self {
            state: Arc::new(MockProviderState {
                responses: Mutex::new(responses),
                requests: Mutex::new(Vec::new()),
            }),
        }
    }

    pub fn recorded_requests(&self) -> Vec<CompletionRequest> {
        self.state.requests.lock().expect("lock requests").clone()
    }
}

#[async_trait]
impl Provider for MockProvider {
    async fn complete(&self, _request: CompletionRequest) -> Result<CompletionResponse, AiError> {
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
                return Err(AiError::StreamError("No mock stream responses left".to_string()));
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

pub struct MockTool {
    name: String,
    description: String,
    execute_fn: Box<dyn Fn(Value) -> anyhow::Result<ToolOutput> + Send + Sync>,
}

impl MockTool {
    pub fn new(
        name: &str,
        execute_fn: Box<dyn Fn(Value) -> anyhow::Result<ToolOutput> + Send + Sync>,
    ) -> Self {
        Self {
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
                "value": { "type": "string" }
            }
        })
    }

    async fn execute(&self, input: Value) -> anyhow::Result<ToolOutput> {
        (self.execute_fn)(input)
    }
}

pub fn text_response(text: &str, stop_reason: &str) -> MockStreamResponse {
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

pub fn tool_use_response(tool_use_id: &str, name: &str, input: Value) -> MockStreamResponse {
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
