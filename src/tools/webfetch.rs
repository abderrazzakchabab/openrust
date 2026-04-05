use std::time::Duration;

use async_trait::async_trait;
use regex::Regex;
use serde_json::{json, Value};

use super::traits::{Tool, ToolOutput};

const FETCH_TIMEOUT_SECS: u64 = 30;
const MAX_OUTPUT_BYTES: usize = 50_000;

pub struct WebFetchTool;

impl WebFetchTool {
    pub fn new() -> Self {
        Self
    }

    /// Strip HTML tags from content
    fn strip_html_tags(html: &str) -> String {
        let tag_regex = Regex::new(r"<[^>]*>").expect("valid regex");
        tag_regex.replace_all(html, "").to_string()
    }

    /// Convert HTML to basic markdown
    fn html_to_markdown(html: &str) -> String {
        let mut result = html.to_string();

        // Headers
        for i in 1..=6 {
            let h_open = format!("<h{i}>");
            let h_close = format!("</h{i}>");
            let h_open_regex = Regex::new(&format!(r"<h{i}[^>]*>")).expect("valid regex");
            let replacement = "#".repeat(i) + " ";
            result = h_open_regex
                .replace_all(&result, replacement.as_str())
                .to_string();
            result = result.replace(&h_close, "\n");
        }

        // Links
        let link_regex =
            Regex::new(r#"<a\s+[^>]*href="([^"]*)"[^>]*>(.*?)</a>"#).expect("valid regex");
        result = link_regex.replace_all(&result, "[$2]($1)").to_string();

        // Paragraphs
        let p_regex = Regex::new(r"<p[^>]*>").expect("valid regex");
        result = p_regex.replace_all(&result, "\n").to_string();
        result = result.replace("</p>", "\n");

        // Lists
        let li_regex = Regex::new(r"<li[^>]*>").expect("valid regex");
        result = li_regex.replace_all(&result, "- ").to_string();
        result = result.replace("</li>", "\n");

        // Line breaks
        let br_regex = Regex::new(r"<br\s*/?>").expect("valid regex");
        result = br_regex.replace_all(&result, "\n").to_string();

        // Strip remaining tags
        Self::strip_html_tags(&result)
    }

    fn truncate_output(output: String) -> String {
        if output.len() <= MAX_OUTPUT_BYTES {
            return output;
        }

        let truncated = String::from_utf8_lossy(&output.as_bytes()[..MAX_OUTPUT_BYTES]).to_string();
        format!("{truncated}\n[output truncated to 50KB]")
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "webfetch"
    }

    fn description(&self) -> &str {
        "Fetch content from a URL and return it in the specified format (text, markdown, or html)"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch content from"
                },
                "format": {
                    "type": "string",
                    "description": "Output format: 'text', 'markdown', or 'html' (default: 'markdown')",
                    "enum": ["text", "markdown", "html"]
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, input: Value) -> anyhow::Result<ToolOutput> {
        let url = match input.get("url").and_then(Value::as_str) {
            Some(url) => url,
            None => return Ok(ToolOutput::error("Missing required field: url")),
        };

        let format = input
            .get("format")
            .and_then(Value::as_str)
            .unwrap_or("markdown");

        if !["text", "markdown", "html"].contains(&format) {
            return Ok(ToolOutput::error(
                "Invalid format. Must be 'text', 'markdown', or 'html'",
            ));
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(FETCH_TIMEOUT_SECS))
            .user_agent("OpenRust/0.1.0 (AI coding assistant)")
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {e}"))?;

        let response = match client.get(url).send().await {
            Ok(resp) => resp,
            Err(e) => {
                return Ok(ToolOutput::error(format!("Failed to fetch URL: {e}")));
            }
        };

        if !response.status().is_success() {
            return Ok(ToolOutput::error(format!(
                "HTTP error: {} {}",
                response.status().as_u16(),
                response.status().canonical_reason().unwrap_or("Unknown")
            )));
        }

        let html = match response.text().await {
            Ok(text) => text,
            Err(e) => {
                return Ok(ToolOutput::error(format!(
                    "Failed to read response body: {e}"
                )));
            }
        };

        let content = match format {
            "html" => html,
            "text" => Self::strip_html_tags(&html),
            "markdown" => Self::html_to_markdown(&html),
            _ => unreachable!(),
        };

        Ok(ToolOutput::success(Self::truncate_output(content)))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn test_webfetch_missing_url() {
        let tool = WebFetchTool::new();
        let output = tool.execute(json!({})).await.expect("execute");

        assert!(output.is_error);
        assert!(output.content.contains("Missing required field: url"));
    }

    #[tokio::test]
    async fn test_webfetch_invalid_format() {
        let tool = WebFetchTool::new();
        let output = tool
            .execute(json!({"url": "https://example.com", "format": "invalid"}))
            .await
            .expect("execute");

        assert!(output.is_error);
        assert!(output.content.contains("Invalid format"));
    }

    #[test]
    fn test_html_tag_stripping() {
        let html = "<p>Hello <strong>world</strong>!</p>";
        let stripped = WebFetchTool::strip_html_tags(html);
        assert_eq!(stripped, "Hello world!");
    }

    #[test]
    fn test_html_to_markdown_headers() {
        let html = "<h1>Title</h1><h2>Subtitle</h2>";
        let markdown = WebFetchTool::html_to_markdown(html);
        assert!(markdown.contains("# Title"));
        assert!(markdown.contains("## Subtitle"));
    }

    #[test]
    fn test_html_to_markdown_links() {
        let html = r#"<a href="https://example.com">Example</a>"#;
        let markdown = WebFetchTool::html_to_markdown(html);
        assert!(markdown.contains("[Example](https://example.com)"));
    }

    #[test]
    fn test_html_to_markdown_lists() {
        let html = "<ul><li>Item 1</li><li>Item 2</li></ul>";
        let markdown = WebFetchTool::html_to_markdown(html);
        assert!(markdown.contains("- Item 1"));
        assert!(markdown.contains("- Item 2"));
    }

    #[test]
    fn test_truncate_output() {
        let short = "short content".to_string();
        assert_eq!(WebFetchTool::truncate_output(short.clone()), short);

        let long = "x".repeat(60_000);
        let truncated = WebFetchTool::truncate_output(long);
        assert!(truncated.len() < 60_000);
        assert!(truncated.contains("[output truncated to 50KB]"));
    }
}
