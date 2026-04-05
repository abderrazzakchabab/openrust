use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::{json, Value};

use super::traits::{Tool, ToolOutput};

pub struct ListTool {
    working_dir: PathBuf,
}

impl ListTool {
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

    fn format_size(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
        let mut size = bytes as f64;
        let mut unit_idx = 0;

        while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
            size /= 1024.0;
            unit_idx += 1;
        }

        if unit_idx == 0 {
            format!("{:.0}{}", size, UNITS[unit_idx])
        } else {
            format!("{:.1}{}", size, UNITS[unit_idx])
        }
    }

    fn list_directory(path: &Path) -> anyhow::Result<String> {
        let mut entries = Vec::new();

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let name = entry.file_name().to_string_lossy().to_string();

            let entry_str = if file_type.is_dir() {
                format!("{}/", name)
            } else if file_type.is_symlink() {
                format!("{}@", name)
            } else {
                let metadata = entry.metadata()?;
                let size = Self::format_size(metadata.len());
                format!("{} ({})", name, size)
            };

            entries.push((name.clone(), entry_str, file_type.is_dir()));
        }

        entries.sort_by(|a, b| match (a.2, b.2) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.0.cmp(&b.0),
        });

        let output = entries
            .into_iter()
            .map(|(_, entry_str, _)| entry_str)
            .collect::<Vec<_>>()
            .join("\n");

        Ok(output)
    }
}

#[async_trait]
impl Tool for ListTool {
    fn name(&self) -> &str {
        "list"
    }

    fn description(&self) -> &str {
        "List directory contents with metadata (name, type, size)"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory path to list, defaults to current directory"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, input: Value) -> anyhow::Result<ToolOutput> {
        let path = input.get("path").and_then(Value::as_str).unwrap_or(".");

        let resolved = self.resolve_path(path);

        if !resolved.exists() {
            return Ok(ToolOutput::error(format!(
                "Path does not exist: {}",
                resolved.display()
            )));
        }

        if !resolved.is_dir() {
            return Ok(ToolOutput::error(format!(
                "Path is not a directory: {}",
                resolved.display()
            )));
        }

        match Self::list_directory(&resolved) {
            Ok(listing) => {
                if listing.is_empty() {
                    Ok(ToolOutput::success("(empty directory)"))
                } else {
                    Ok(ToolOutput::success(listing))
                }
            }
            Err(e) => Ok(ToolOutput::error(format!(
                "Failed to list directory: {}",
                e
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_list_directory_with_files_and_subdirs() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        fs::create_dir(temp_path.join("subdir1")).unwrap();
        fs::create_dir(temp_path.join("subdir2")).unwrap();
        fs::write(temp_path.join("file1.txt"), "content1").unwrap();
        fs::write(temp_path.join("file2.txt"), "content2").unwrap();

        let tool = ListTool::new(temp_path.to_path_buf());
        let result = tool.execute(json!({})).await.unwrap();

        assert!(!result.is_error);
        let content = result.content;

        let lines: Vec<&str> = content.lines().collect();
        assert!(lines[0].contains("subdir"));
        assert!(lines[1].contains("subdir"));
        assert!(lines[2].contains("file"));
        assert!(lines[3].contains("file"));
    }

    #[tokio::test]
    async fn test_list_dirs_first_sorting() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        fs::write(temp_path.join("aaa_file.txt"), "content").unwrap();
        fs::create_dir(temp_path.join("zzz_dir")).unwrap();
        fs::write(temp_path.join("bbb_file.txt"), "content").unwrap();
        fs::create_dir(temp_path.join("aaa_dir")).unwrap();

        let tool = ListTool::new(temp_path.to_path_buf());
        let result = tool.execute(json!({})).await.unwrap();

        assert!(!result.is_error);
        let lines: Vec<&str> = result.content.lines().collect();

        assert!(lines[0].starts_with("aaa_dir"));
        assert!(lines[1].starts_with("zzz_dir"));
        assert!(lines[2].starts_with("aaa_file"));
        assert!(lines[3].starts_with("bbb_file"));
    }

    #[tokio::test]
    async fn test_list_nonexistent_path() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let tool = ListTool::new(temp_path.to_path_buf());
        let result = tool
            .execute(json!({"path": "/nonexistent/path/that/does/not/exist"}))
            .await
            .unwrap();

        assert!(result.is_error);
        assert!(result.content.contains("does not exist"));
    }

    #[tokio::test]
    async fn test_list_file_not_directory() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        let file_path = temp_path.join("test.txt");
        fs::write(&file_path, "content").unwrap();

        let tool = ListTool::new(temp_path.to_path_buf());
        let result = tool.execute(json!({"path": "test.txt"})).await.unwrap();

        assert!(result.is_error);
        assert!(result.content.contains("not a directory"));
    }

    #[tokio::test]
    async fn test_list_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let tool = ListTool::new(temp_path.to_path_buf());
        let result = tool.execute(json!({})).await.unwrap();

        assert!(!result.is_error);
        assert_eq!(result.content, "(empty directory)");
    }

    #[tokio::test]
    async fn test_list_with_relative_path() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let subdir = temp_path.join("subdir");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("file.txt"), "content").unwrap();

        let tool = ListTool::new(temp_path.to_path_buf());
        let result = tool.execute(json!({"path": "subdir"})).await.unwrap();

        assert!(!result.is_error);
        assert!(result.content.contains("file.txt"));
    }

    #[test]
    fn test_format_size() {
        assert_eq!(ListTool::format_size(0), "0B");
        assert_eq!(ListTool::format_size(512), "512B");
        assert_eq!(ListTool::format_size(1024), "1.0KB");
        assert_eq!(ListTool::format_size(1536), "1.5KB");
        assert_eq!(ListTool::format_size(1048576), "1.0MB");
        assert_eq!(ListTool::format_size(1572864), "1.5MB");
    }
}
