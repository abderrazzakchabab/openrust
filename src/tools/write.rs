use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::{json, Value};

use super::traits::{Tool, ToolOutput};

pub struct WriteTool {
    working_dir: PathBuf,
}

impl WriteTool {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        let path = Path::new(path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.working_dir.join(path)
        }
    }
}

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn description(&self) -> &str {
        "Write text content to a file and create parent directories"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "File contents to write"
                }
            },
            "required": ["file_path", "content"]
        })
    }

    async fn execute(&self, input: Value) -> anyhow::Result<ToolOutput> {
        let file_path = match input.get("file_path").and_then(Value::as_str) {
            Some(file_path) => file_path,
            None => return Ok(ToolOutput::error("Missing required field: file_path")),
        };

        let content = match input.get("content").and_then(Value::as_str) {
            Some(content) => content,
            None => return Ok(ToolOutput::error("Missing required field: content")),
        };

        let resolved = self.resolve_path(file_path);

        if let Some(parent) = resolved.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&resolved, content)?;

        Ok(ToolOutput::success(format!(
            "Successfully wrote {} bytes to {}",
            content.len(),
            resolved.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use super::*;

    fn create_temp_dir() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time ok")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("openrust-write-test-{unique}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[tokio::test]
    async fn test_write_file() {
        let dir = create_temp_dir();
        let tool = WriteTool::new(dir.clone());

        let output = tool
            .execute(json!({"file_path": "note.txt", "content": "hello"}))
            .await
            .expect("execute");

        assert!(!output.is_error);
        assert!(output.content.contains("Successfully wrote 5 bytes"));
        assert_eq!(
            fs::read_to_string(dir.join("note.txt")).expect("read"),
            "hello"
        );
    }

    #[tokio::test]
    async fn test_write_creates_dirs() {
        let dir = create_temp_dir();
        let tool = WriteTool::new(dir.clone());

        let output = tool
            .execute(json!({
                "file_path": "nested/path/file.txt",
                "content": "created"
            }))
            .await
            .expect("execute");

        assert!(!output.is_error);
        assert_eq!(
            fs::read_to_string(dir.join("nested/path/file.txt")).expect("read"),
            "created"
        );
    }
}
