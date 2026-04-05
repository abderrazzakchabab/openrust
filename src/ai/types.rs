use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
            Role::System => write!(f, "system"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
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
        #[serde(default)]
        is_error: bool,
    },
    Thinking {
        thinking: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentDelta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Usage {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    ContentBlockStart {
        index: u32,
        content_block: ContentBlock,
    },
    ContentBlockDelta {
        index: u32,
        delta: ContentDelta,
    },
    ContentBlockStop {
        index: u32,
    },
    MessageStart,
    MessageDelta {
        stop_reason: Option<String>,
        usage: Option<Usage>,
    },
    MessageStop,
    Error {
        message: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MessageContentInput {
    LegacyText(String),
    Blocks(std::vec::Vec<ContentBlock>),
}

fn deserialize_content_blocks<'de, D>(
    deserializer: D,
) -> Result<std::vec::Vec<ContentBlock>, D::Error>
where
    D: Deserializer<'de>,
{
    match MessageContentInput::deserialize(deserializer)? {
        MessageContentInput::LegacyText(text) => Ok(std::vec![ContentBlock::Text { text }]),
        MessageContentInput::Blocks(blocks) => Ok(blocks),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    #[serde(deserialize_with = "deserialize_content_blocks")]
    pub content: std::vec::Vec<ContentBlock>,
}

impl Message {
    pub fn text_content(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>()
    }

    pub fn new_user(text: impl Into<String>) -> Self {
        Message {
            role: Role::User,
            content: std::vec![ContentBlock::Text { text: text.into() }],
        }
    }

    pub fn new_assistant(text: impl Into<String>) -> Self {
        Message {
            role: Role::Assistant,
            content: std::vec![ContentBlock::Text { text: text.into() }],
        }
    }

    pub fn append_text(&mut self, chunk: &str) {
        if let Some(ContentBlock::Text { text }) = self.content.last_mut() {
            text.push_str(chunk);
            return;
        }

        self.content.push(ContentBlock::Text {
            text: chunk.to_string(),
        });
    }
}

#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub messages: std::vec::Vec<Message>,
    pub model: String,
    pub max_tokens: u32,
    pub system: Option<String>,
    pub stream: bool,
    pub tools: Option<std::vec::Vec<ToolDefinition>>,
}

#[derive(Debug, Clone)]
pub struct CompletionResponse {
    pub content: std::vec::Vec<ContentBlock>,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub model: String,
    pub stop_reason: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("API key not configured for provider: {0}")]
    MissingApiKey(String),

    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("Failed to parse API response: {0}")]
    ParseError(String),

    #[error("API error {status}: {message}")]
    ApiError { status: u16, message: String },

    #[error("Stream error: {0}")]
    StreamError(String),
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ContentBlock, Message, Role};

    #[test]
    fn content_block_roundtrip_serialization() {
        let blocks = vec![
            ContentBlock::Text {
                text: "hello".to_string(),
            },
            ContentBlock::ToolUse {
                id: "toolu_01".to_string(),
                name: "bash".to_string(),
                input: json!({ "command": "ls" }),
            },
            ContentBlock::ToolResult {
                tool_use_id: "toolu_01".to_string(),
                content: "ok".to_string(),
                is_error: false,
            },
            ContentBlock::Thinking {
                thinking: "checking".to_string(),
            },
        ];

        let serialized = serde_json::to_string(&blocks).expect("serialize content blocks");
        let deserialized: Vec<ContentBlock> =
            serde_json::from_str(&serialized).expect("deserialize content blocks");

        assert_eq!(deserialized, blocks);
    }

    #[test]
    fn message_deserializes_legacy_string_content() {
        let value = json!({
            "role": "user",
            "content": "legacy text"
        });

        let message: Message = serde_json::from_value(value).expect("deserialize message");

        assert_eq!(message.role, Role::User);
        assert_eq!(
            message.content,
            vec![ContentBlock::Text {
                text: "legacy text".to_string()
            }]
        );
    }

    #[test]
    fn message_deserializes_block_content() {
        let value = json!({
            "role": "assistant",
            "content": [
                {"type": "text", "text": "first"},
                {"type": "text", "text": " second"}
            ]
        });

        let message: Message = serde_json::from_value(value).expect("deserialize message");

        assert_eq!(message.role, Role::Assistant);
        assert_eq!(message.text_content(), "first second");
    }
}
