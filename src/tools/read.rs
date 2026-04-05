use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::{json, Value};

use super::traits::{Tool, ToolOutput};

const DEFAULT_LIMIT: usize = 2000;
const MAX_LINE_LEN: usize = 2000;
const BINARY_CHECK_BYTES: usize = 8192;

pub struct ReadTool {
    working_dir: PathBuf,
}

impl ReadTool {
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

    fn read_directory(path: &Path) -> anyhow::Result<String> {
        let mut entries = Vec::new();

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let mut name = entry.file_name().to_string_lossy().to_string();
            if file_type.is_dir() {
                name.push('/');
            }
            entries.push(name);
        }

        entries.sort();
        Ok(entries.join("\n"))
    }

    fn detect_binary(bytes: &[u8]) -> bool {
        bytes.iter().any(|byte| *byte == 0)
    }

    fn truncate_line(line: &str) -> String {
        if line.chars().count() <= MAX_LINE_LEN {
            return line.to_string();
        }

        let truncated: String = line.chars().take(MAX_LINE_LEN).collect();
        format!("{truncated}[truncated]")
    }

    fn read_file(path: &Path, offset: usize, limit: usize) -> anyhow::Result<ToolOutput> {
        let bytes = std::fs::read(path)?;
        let check_len = bytes.len().min(BINARY_CHECK_BYTES);
        if Self::detect_binary(&bytes[..check_len]) {
            return Ok(ToolOutput::error("Binary file detected"));
        }

        let content = String::from_utf8_lossy(&bytes).to_string();
        let start = offset.saturating_sub(1);

        let output = content
            .lines()
            .enumerate()
            .skip(start)
            .take(limit)
            .map(|(idx, line)| format!("{}: {}", idx + 1, Self::truncate_line(line)))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolOutput::success(output))
    }
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn description(&self) -> &str {
        "Read files with line numbers or list directory contents"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file or directory"
                },
                "offset": {
                    "type": "integer",
                    "description": "1-indexed line offset for file reads"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read (default 2000)"
                }
            },
            "required": ["file_path"]
        })
    }

    async fn execute(&self, input: Value) -> anyhow::Result<ToolOutput> {
        let file_path = match input.get("file_path").and_then(Value::as_str) {
            Some(file_path) => file_path,
            None => return Ok(ToolOutput::error("Missing required field: file_path")),
        };

        let offset = input
            .get("offset")
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .max(1) as usize;
        let limit = input
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_LIMIT as u64) as usize;

        let resolved = self.resolve_path(file_path);

        if resolved.is_dir() {
            let listing = Self::read_directory(&resolved)?;
            return Ok(ToolOutput::success(listing));
        }

        if !resolved.exists() {
            return Ok(ToolOutput::error(format!(
                "Path does not exist: {}",
                resolved.display()
            )));
        }

        Self::read_file(&resolved, offset, limit)
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
        let dir = std::env::temp_dir().join(format!("openrust-read-test-{unique}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[tokio::test]
    async fn test_read_file() {
        let dir = create_temp_dir();
        let file = dir.join("sample.txt");
        fs::write(&file, "one\ntwo\nthree\n").expect("write file");

        let tool = ReadTool::new(dir);
        let output = tool
            .execute(json!({"file_path": "sample.txt"}))
            .await
            .expect("execute");

        assert!(!output.is_error);
        assert!(output.content.contains("1: one"));
        assert!(output.content.contains("2: two"));
        assert!(output.content.contains("3: three"));
    }

    #[tokio::test]
    async fn test_read_offset_limit() {
        let dir = create_temp_dir();
        let file = dir.join("sample.txt");
        fs::write(&file, "1\n2\n3\n4\n5\n").expect("write file");

        let tool = ReadTool::new(dir);
        let output = tool
            .execute(json!({"file_path": "sample.txt", "offset": 3, "limit": 2}))
            .await
            .expect("execute");

        assert!(!output.is_error);
        assert_eq!(output.content, "3: 3\n4: 4");
    }

    #[tokio::test]
    async fn test_read_directory() {
        let dir = create_temp_dir();
        fs::write(dir.join("a.txt"), "a").expect("write file");
        fs::create_dir_all(dir.join("nested")).expect("create nested");

        let tool = ReadTool::new(dir.clone());
        let output = tool
            .execute(json!({"file_path": dir.to_string_lossy().to_string()}))
            .await
            .expect("execute");

        assert!(!output.is_error);
        assert!(output.content.contains("a.txt"));
        assert!(output.content.contains("nested/"));
    }
}
