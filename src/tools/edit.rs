use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::{json, Value};

use super::traits::{Tool, ToolOutput};

pub struct EditTool {
    working_dir: PathBuf,
}

impl EditTool {
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
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "Find and replace text in a file"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "Text to find"
                },
                "new_string": {
                    "type": "string",
                    "description": "Replacement text"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences (default false)"
                }
            },
            "required": ["file_path", "old_string", "new_string"]
        })
    }

    async fn execute(&self, input: Value) -> anyhow::Result<ToolOutput> {
        let file_path = match input.get("file_path").and_then(Value::as_str) {
            Some(file_path) => file_path,
            None => return Ok(ToolOutput::error("Missing required field: file_path")),
        };

        let old_string = match input.get("old_string").and_then(Value::as_str) {
            Some(old_string) => old_string,
            None => return Ok(ToolOutput::error("Missing required field: old_string")),
        };

        let new_string = match input.get("new_string").and_then(Value::as_str) {
            Some(new_string) => new_string,
            None => return Ok(ToolOutput::error("Missing required field: new_string")),
        };

        let replace_all = input
            .get("replace_all")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let resolved = self.resolve_path(file_path);
        let content = std::fs::read_to_string(&resolved)?;

        let matches = content.matches(old_string).count();
        if matches == 0 {
            return Ok(ToolOutput::error("oldString not found in file"));
        }

        if matches > 1 && !replace_all {
            return Ok(ToolOutput::error(format!(
                "Found {matches} matches for oldString. Use replace_all=true to replace all, or provide more context to match uniquely."
            )));
        }

        let updated = if replace_all {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };

        let replaced_count = if replace_all { matches } else { 1 };
        std::fs::write(&resolved, updated)?;

        Ok(ToolOutput::success(format!(
            "Successfully edited {} (replaced {replaced_count} occurrence(s))",
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
        let dir = std::env::temp_dir().join(format!("openrust-edit-test-{unique}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[tokio::test]
    async fn test_edit_single_match() {
        let dir = create_temp_dir();
        let file = dir.join("sample.txt");
        fs::write(&file, "hello world").expect("write file");

        let tool = EditTool::new(dir.clone());
        let output = tool
            .execute(json!({
                "file_path": "sample.txt",
                "old_string": "world",
                "new_string": "rust"
            }))
            .await
            .expect("execute");

        assert!(!output.is_error);
        assert_eq!(fs::read_to_string(file).expect("read"), "hello rust");
    }

    #[tokio::test]
    async fn test_edit_not_found() {
        let dir = create_temp_dir();
        let file = dir.join("sample.txt");
        fs::write(&file, "hello").expect("write file");

        let tool = EditTool::new(dir);
        let output = tool
            .execute(json!({
                "file_path": "sample.txt",
                "old_string": "missing",
                "new_string": "new"
            }))
            .await
            .expect("execute");

        assert!(output.is_error);
        assert!(output.content.contains("oldString not found"));
    }

    #[tokio::test]
    async fn test_edit_multiple_matches_error() {
        let dir = create_temp_dir();
        let file = dir.join("sample.txt");
        fs::write(&file, "x x x").expect("write file");

        let tool = EditTool::new(dir);
        let output = tool
            .execute(json!({
                "file_path": "sample.txt",
                "old_string": "x",
                "new_string": "y"
            }))
            .await
            .expect("execute");

        assert!(output.is_error);
        assert!(output.content.contains("Found 3 matches for oldString"));
    }

    #[tokio::test]
    async fn test_edit_replace_all() {
        let dir = create_temp_dir();
        let file = dir.join("sample.txt");
        fs::write(&file, "x x x").expect("write file");

        let tool = EditTool::new(dir.clone());
        let output = tool
            .execute(json!({
                "file_path": "sample.txt",
                "old_string": "x",
                "new_string": "y",
                "replace_all": true
            }))
            .await
            .expect("execute");

        assert!(!output.is_error);
        assert_eq!(fs::read_to_string(file).expect("read"), "y y y");
        assert!(output.content.contains("replaced 3 occurrence(s)"));
    }
}
