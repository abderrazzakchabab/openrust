use crate::ai::types::{ContentBlock, Message};

/// Approximate token count for a message (rough: ~4 chars per token)
pub fn estimate_tokens(messages: &[Message]) -> usize {
    messages
        .iter()
        .map(|m| {
            m.content
                .iter()
                .map(|block| match block {
                    ContentBlock::Text { text } => text.len() / 4,
                    ContentBlock::ToolUse { input, .. } => input.to_string().len() / 4,
                    ContentBlock::ToolResult { content, .. } => content.len() / 4,
                    ContentBlock::Thinking { thinking } => thinking.len() / 4,
                })
                .sum::<usize>()
        })
        .sum()
}

/// Check if conversation needs compaction
pub fn needs_compaction(messages: &[Message], max_context_tokens: usize) -> bool {
    let estimated = estimate_tokens(messages);
    let threshold = max_context_tokens * 80 / 100;
    estimated > threshold
}

/// Create compaction request messages
pub fn create_compaction_request(messages: &[Message]) -> Vec<Message> {
    let conversation_text = messages
        .iter()
        .map(|m| format!("{}: {}", m.role, m.text_content()))
        .collect::<Vec<_>>()
        .join("\n\n");

    vec![Message::new_user(format!(
        "Please summarize the following conversation, preserving all critical context:\n\n{}",
        conversation_text
    ))]
}

/// Create title generation request
pub fn create_title_request(messages: &[Message]) -> Vec<Message> {
    let first_messages: Vec<_> = messages.iter().take(4).collect();
    let context = first_messages
        .iter()
        .map(|m| format!("{}: {}", m.role, m.text_content()))
        .collect::<Vec<_>>()
        .join("\n");

    vec![Message::new_user(format!(
        "Generate a short title for this conversation:\n\n{}",
        context
    ))]
}

/// Create summary generation request
pub fn create_summary_request(messages: &[Message]) -> Vec<Message> {
    let context = messages
        .iter()
        .map(|m| format!("{}: {}", m.role, m.text_content()))
        .collect::<Vec<_>>()
        .join("\n\n");

    vec![Message::new_user(format!(
        "Summarize what was accomplished:\n\n{}",
        context
    ))]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::types::Role;

    #[test]
    fn test_estimate_tokens_returns_reasonable_count() {
        let messages = vec![
            Message::new_user("Hello world"),
            Message::new_assistant("Hi there, how can I help you?"),
        ];

        let token_count = estimate_tokens(&messages);

        assert!(token_count >= 8 && token_count <= 12);
    }

    #[test]
    fn test_needs_compaction_triggers_at_80_percent() {
        let messages = vec![
            Message::new_user("a".repeat(3200)),
            Message::new_assistant("b".repeat(400)),
        ];

        assert!(needs_compaction(&messages, 1000));
        assert!(!needs_compaction(&messages, 2000));
    }

    #[test]
    fn test_create_compaction_request_includes_conversation() {
        let messages = vec![
            Message::new_user("First message"),
            Message::new_assistant("First response"),
        ];

        let request = create_compaction_request(&messages);

        assert_eq!(request.len(), 1);
        assert_eq!(request[0].role, Role::User);

        let content = request[0].text_content();
        assert!(content.contains("Please summarize"));
        assert!(content.contains("First message"));
        assert!(content.contains("First response"));
    }

    #[test]
    fn test_create_title_request_uses_first_messages() {
        let messages = vec![
            Message::new_user("User message 1"),
            Message::new_assistant("Assistant message 1"),
            Message::new_user("User message 2"),
            Message::new_assistant("Assistant message 2"),
            Message::new_user("User message 3"),
            Message::new_user("User message 4"),
        ];

        let request = create_title_request(&messages);

        assert_eq!(request.len(), 1);
        assert_eq!(request[0].role, Role::User);

        let content = request[0].text_content();
        assert!(content.contains("Generate a short title"));
        assert!(content.contains("User message 1"));
        assert!(content.contains("Assistant message 2"));
        assert!(!content.contains("User message 4"));
    }

    #[test]
    fn test_create_summary_request_includes_all_messages() {
        let messages = vec![
            Message::new_user("Start"),
            Message::new_assistant("Middle"),
            Message::new_user("End"),
        ];

        let request = create_summary_request(&messages);

        assert_eq!(request.len(), 1);
        assert_eq!(request[0].role, Role::User);

        let content = request[0].text_content();
        assert!(content.contains("Summarize what was accomplished"));
        assert!(content.contains("Start"));
        assert!(content.contains("Middle"));
        assert!(content.contains("End"));
    }

    #[test]
    fn test_estimate_tokens_handles_mixed_content_blocks() {
        use serde_json::json;

        let mut message = Message::new_user("Text part");
        message.content.push(ContentBlock::ToolUse {
            id: "tool_1".to_string(),
            name: "bash".to_string(),
            input: json!({"command": "ls"}),
        });
        message.content.push(ContentBlock::ToolResult {
            tool_use_id: "tool_1".to_string(),
            content: "file1.txt\nfile2.txt".to_string(),
            is_error: false,
        });
        message.content.push(ContentBlock::Thinking {
            thinking: "Let me check the files".to_string(),
        });

        let messages = vec![message];
        let token_count = estimate_tokens(&messages);

        assert!(token_count > 0);
    }
}
