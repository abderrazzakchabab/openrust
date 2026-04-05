use std::fs;
use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::traits::{Tool, ToolOutput};

const MAX_RESULTS: usize = 100;

pub struct GlobTool {
    working_dir: PathBuf,
}

impl GlobTool {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern, sorted by modification time (newest first), limited to 100 results"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match files (e.g., '**/*.rs', '*.txt')"
                },
                "path": {
                    "type": "string",
                    "description": "Root directory to search in (default: '.')"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, input: Value) -> anyhow::Result<ToolOutput> {
        let pattern = match input.get("pattern").and_then(Value::as_str) {
            Some(p) => p,
            None => return Ok(ToolOutput::error("Missing required field: pattern")),
        };

        let root_path = input.get("path").and_then(Value::as_str).unwrap_or(".");

        // Resolve the root path relative to working_dir
        let search_root = if std::path::Path::new(root_path).is_absolute() {
            PathBuf::from(root_path)
        } else {
            self.working_dir.join(root_path)
        };

        // Build the full glob pattern
        let full_pattern = if pattern.starts_with('/') {
            pattern.to_string()
        } else {
            format!("{}/{}", search_root.display(), pattern)
        };

        // Execute glob pattern matching
        let glob_results = match ::glob::glob(&full_pattern) {
            Ok(paths) => paths,
            Err(e) => return Ok(ToolOutput::error(format!("Invalid glob pattern: {}", e))),
        };

        // Collect results with modification times
        let mut results: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

        for path_result in glob_results {
            match path_result {
                Ok(path) => {
                    // Get modification time for sorting
                    match fs::metadata(&path) {
                        Ok(metadata) => {
                            if let Ok(modified) = metadata.modified() {
                                results.push((path, modified));
                            }
                        }
                        Err(_) => {
                            // Skip files we can't stat
                            continue;
                        }
                    }
                }
                Err(_) => {
                    // Skip invalid paths
                    continue;
                }
            }
        }

        // Sort by modification time (newest first)
        results.sort_by(|a, b| b.1.cmp(&a.1));

        // Limit to MAX_RESULTS
        results.truncate(MAX_RESULTS);

        // Format output: one path per line
        let output = results
            .iter()
            .map(|(path, _)| path.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");

        if output.is_empty() {
            Ok(ToolOutput::success("(no matches)"))
        } else {
            Ok(ToolOutput::success(output))
        }
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
        let dir = std::env::temp_dir().join(format!("openrust-glob-test-{unique}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[tokio::test]
    async fn test_glob_basic_pattern() {
        let temp_dir = create_temp_dir();
        let tool = GlobTool::new(temp_dir.clone());

        // Create test files
        fs::write(temp_dir.join("file1.rs"), "content1").expect("write file1");
        fs::write(temp_dir.join("file2.rs"), "content2").expect("write file2");
        fs::write(temp_dir.join("file3.txt"), "content3").expect("write file3");

        let output = tool
            .execute(json!({"pattern": "*.rs"}))
            .await
            .expect("execute");

        assert!(!output.is_error);
        assert!(output.content.contains("file1.rs"));
        assert!(output.content.contains("file2.rs"));
        assert!(!output.content.contains("file3.txt"));

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_glob_sorting_by_mtime() {
        let temp_dir = create_temp_dir();
        let tool = GlobTool::new(temp_dir.clone());

        // Create files with slight delays to ensure different mtimes
        fs::write(temp_dir.join("old.txt"), "old").expect("write old");
        std::thread::sleep(std::time::Duration::from_millis(10));
        fs::write(temp_dir.join("new.txt"), "new").expect("write new");

        let output = tool
            .execute(json!({"pattern": "*.txt"}))
            .await
            .expect("execute");

        assert!(!output.is_error);
        let lines: Vec<&str> = output.content.lines().collect();
        assert_eq!(lines.len(), 2);
        // Newest first: new.txt should come before old.txt
        assert!(lines[0].contains("new.txt"));
        assert!(lines[1].contains("old.txt"));

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_glob_result_limit() {
        let temp_dir = create_temp_dir();
        let tool = GlobTool::new(temp_dir.clone());

        // Create more than MAX_RESULTS files
        for i in 0..150 {
            fs::write(temp_dir.join(format!("file{:03}.txt", i)), "content").expect("write file");
        }

        let output = tool
            .execute(json!({"pattern": "*.txt"}))
            .await
            .expect("execute");

        assert!(!output.is_error);
        let lines: Vec<&str> = output.content.lines().collect();
        assert_eq!(lines.len(), MAX_RESULTS);

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_glob_no_matches() {
        let temp_dir = create_temp_dir();
        let tool = GlobTool::new(temp_dir.clone());

        fs::write(temp_dir.join("file.txt"), "content").expect("write file");

        let output = tool
            .execute(json!({"pattern": "*.rs"}))
            .await
            .expect("execute");

        assert!(!output.is_error);
        assert_eq!(output.content, "(no matches)");

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_glob_missing_pattern() {
        let temp_dir = create_temp_dir();
        let tool = GlobTool::new(temp_dir.clone());

        let output = tool.execute(json!({})).await.expect("execute");

        assert!(output.is_error);
        assert!(output.content.contains("Missing required field: pattern"));

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_glob_with_path_parameter() {
        let temp_dir = create_temp_dir();
        let tool = GlobTool::new(temp_dir.clone());

        // Create a subdirectory with files
        let subdir = temp_dir.join("subdir");
        fs::create_dir_all(&subdir).expect("create subdir");
        fs::write(subdir.join("nested.rs"), "content").expect("write nested");

        let output = tool
            .execute(json!({"pattern": "*.rs", "path": "subdir"}))
            .await
            .expect("execute");

        assert!(!output.is_error);
        assert!(output.content.contains("nested.rs"));

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }
}
