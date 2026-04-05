use std::fs;
use std::path::PathBuf;

use async_trait::async_trait;
use regex::Regex;
use serde_json::{json, Value};

use super::traits::{Tool, ToolOutput};

const MAX_FILES: usize = 100;
const MAX_MATCHES: usize = 1000;
const MAX_LINE_LENGTH: usize = 500;
const BINARY_CHECK_SIZE: usize = 8192;

pub struct GrepTool {
    working_dir: PathBuf,
}

impl GrepTool {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        let p = PathBuf::from(path);
        if p.is_absolute() {
            p
        } else {
            self.working_dir.join(p)
        }
    }

    fn should_skip_dir(name: &str) -> bool {
        matches!(
            name,
            ".git" | ".svn" | ".hg" | "target" | "node_modules" | ".DS_Store"
        )
    }

    fn is_binary_file(path: &PathBuf) -> bool {
        match fs::read(path) {
            Ok(content) => {
                let check_len = std::cmp::min(content.len(), BINARY_CHECK_SIZE);
                content[..check_len].iter().any(|&b| b == 0)
            }
            Err(_) => true,
        }
    }

    fn matches_include_filter(filename: &str, include: &str) -> bool {
        if include.starts_with("*.") {
            let ext = &include[1..];
            filename.ends_with(ext)
        } else if include.contains('*') {
            let pattern = include.replace("*", ".*");
            if let Ok(re) = Regex::new(&format!("^{}$", pattern)) {
                re.is_match(filename)
            } else {
                false
            }
        } else {
            filename == include
        }
    }

    fn truncate_line(line: &str) -> String {
        if line.len() <= MAX_LINE_LENGTH {
            line.to_string()
        } else {
            format!("{}...", &line[..MAX_LINE_LENGTH])
        }
    }

    fn search_directory(
        &self,
        dir: &PathBuf,
        regex: &Regex,
        include_filter: &Option<String>,
        results: &mut Vec<String>,
        file_count: &mut usize,
        match_count: &mut usize,
    ) -> anyhow::Result<()> {
        if *file_count >= MAX_FILES || *match_count >= MAX_MATCHES {
            return Ok(());
        }

        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };

        for entry in entries {
            if *file_count >= MAX_FILES || *match_count >= MAX_MATCHES {
                break;
            }

            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            let file_name = match path.file_name() {
                Some(n) => n.to_string_lossy().to_string(),
                None => continue,
            };

            if Self::should_skip_dir(&file_name) {
                continue;
            }

            if path.is_dir() {
                let _ = self.search_directory(
                    &path,
                    regex,
                    include_filter,
                    results,
                    file_count,
                    match_count,
                );
            } else if path.is_file() {
                if let Some(filter) = include_filter {
                    if !Self::matches_include_filter(&file_name, filter) {
                        continue;
                    }
                }

                if Self::is_binary_file(&path) {
                    continue;
                }

                if let Ok(content) = fs::read_to_string(&path) {
                    for (line_num, line) in content.lines().enumerate() {
                        if *match_count >= MAX_MATCHES {
                            break;
                        }

                        if regex.is_match(line) {
                            let truncated = Self::truncate_line(line);
                            let result =
                                format!("{}:{}: {}", path.display(), line_num + 1, truncated);
                            results.push(result);
                            *match_count += 1;
                        }
                    }

                    *file_count += 1;
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search for a regex pattern in files across a directory tree"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "Root directory to search (default: current directory)"
                },
                "include": {
                    "type": "string",
                    "description": "File glob filter (e.g., '*.rs', '*.txt')"
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

        let regex = match Regex::new(pattern) {
            Ok(r) => r,
            Err(e) => return Ok(ToolOutput::error(format!("Invalid regex pattern: {}", e))),
        };

        let path_str = input.get("path").and_then(Value::as_str).unwrap_or(".");

        let search_path = self.resolve_path(path_str);

        if !search_path.exists() {
            return Ok(ToolOutput::error(format!(
                "Path does not exist: {}",
                search_path.display()
            )));
        }

        let include_filter = input
            .get("include")
            .and_then(Value::as_str)
            .map(|s| s.to_string());

        let mut results = Vec::new();
        let mut file_count = 0;
        let mut match_count = 0;

        self.search_directory(
            &search_path,
            &regex,
            &include_filter,
            &mut results,
            &mut file_count,
            &mut match_count,
        )?;

        if results.is_empty() {
            return Ok(ToolOutput::success("No matches found"));
        }

        let mut output = format!("Found {} matches in {} files:\n\n", match_count, file_count);
        output.push_str(&results.join("\n"));

        if match_count >= MAX_MATCHES {
            output.push_str(&format!(
                "\n\n[Results truncated at {} matches]",
                MAX_MATCHES
            ));
        }

        Ok(ToolOutput::success(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_files(dir: &TempDir) -> anyhow::Result<()> {
        let dir_path = dir.path();

        fs::write(
            dir_path.join("test1.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n",
        )?;
        fs::write(
            dir_path.join("test2.rs"),
            "fn test() {\n    assert_eq!(1, 1);\n}\n",
        )?;
        fs::write(
            dir_path.join("readme.txt"),
            "This is a readme\nWith some content\n",
        )?;

        fs::create_dir(dir_path.join("subdir"))?;
        fs::write(
            dir_path.join("subdir/nested.rs"),
            "fn nested() {\n    println!(\"nested\");\n}\n",
        )?;

        fs::create_dir(dir_path.join(".git"))?;
        fs::write(dir_path.join(".git/config"), "git config\n")?;

        Ok(())
    }

    #[tokio::test]
    async fn test_grep_basic_regex_match() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(&temp_dir).unwrap();

        let grep = GrepTool::new(temp_dir.path().to_path_buf());
        let input = json!({
            "pattern": "fn.*\\(",
            "path": "."
        });

        let result = grep.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("fn main()"));
        assert!(result.content.contains("fn test()"));
        assert!(result.content.contains("fn nested()"));
    }

    #[tokio::test]
    async fn test_grep_with_include_filter() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(&temp_dir).unwrap();

        let grep = GrepTool::new(temp_dir.path().to_path_buf());
        let input = json!({
            "pattern": "println",
            "path": ".",
            "include": "*.rs"
        });

        let result = grep.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("println"));
        assert!(!result.content.contains("readme.txt"));
    }

    #[tokio::test]
    async fn test_grep_skips_hidden_dirs() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(&temp_dir).unwrap();

        let grep = GrepTool::new(temp_dir.path().to_path_buf());
        let input = json!({
            "pattern": "git config",
            "path": "."
        });

        let result = grep.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("No matches found"));
    }

    #[tokio::test]
    async fn test_grep_invalid_regex() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(&temp_dir).unwrap();

        let grep = GrepTool::new(temp_dir.path().to_path_buf());
        let input = json!({
            "pattern": "[invalid(regex",
            "path": "."
        });

        let result = grep.execute(input).await.unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("Invalid regex pattern"));
    }

    #[tokio::test]
    async fn test_grep_missing_pattern() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(&temp_dir).unwrap();

        let grep = GrepTool::new(temp_dir.path().to_path_buf());
        let input = json!({
            "path": "."
        });

        let result = grep.execute(input).await.unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("Missing required field: pattern"));
    }

    #[tokio::test]
    async fn test_grep_nonexistent_path() {
        let temp_dir = TempDir::new().unwrap();
        let grep = GrepTool::new(temp_dir.path().to_path_buf());
        let input = json!({
            "pattern": "test",
            "path": "/nonexistent/path"
        });

        let result = grep.execute(input).await.unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("Path does not exist"));
    }

    #[test]
    fn test_should_skip_dir() {
        assert!(GrepTool::should_skip_dir(".git"));
        assert!(GrepTool::should_skip_dir(".svn"));
        assert!(GrepTool::should_skip_dir("target"));
        assert!(GrepTool::should_skip_dir("node_modules"));
        assert!(!GrepTool::should_skip_dir("src"));
        assert!(!GrepTool::should_skip_dir("tests"));
    }

    #[test]
    fn test_matches_include_filter() {
        assert!(GrepTool::matches_include_filter("test.rs", "*.rs"));
        assert!(GrepTool::matches_include_filter("main.rs", "*.rs"));
        assert!(!GrepTool::matches_include_filter("test.txt", "*.rs"));
        assert!(GrepTool::matches_include_filter("readme.txt", "*.txt"));
    }

    #[test]
    fn test_truncate_line() {
        let short = "short line";
        assert_eq!(GrepTool::truncate_line(short), short);

        let long = "a".repeat(600);
        let truncated = GrepTool::truncate_line(&long);
        assert!(truncated.ends_with("..."));
        assert!(truncated.len() < long.len());
    }
}
