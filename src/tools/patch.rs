use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::{json, Value};

use super::traits::{Tool, ToolOutput};

pub struct PatchTool {
    working_dir: PathBuf,
}

impl PatchTool {
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
impl Tool for PatchTool {
    fn name(&self) -> &str {
        "patch"
    }

    fn description(&self) -> &str {
        "Apply a unified diff patch to a file"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to patch"
                },
                "diff": {
                    "type": "string",
                    "description": "Unified diff format patch to apply"
                }
            },
            "required": ["file_path", "diff"]
        })
    }

    async fn execute(&self, input: Value) -> anyhow::Result<ToolOutput> {
        let file_path = match input.get("file_path").and_then(Value::as_str) {
            Some(file_path) => file_path,
            None => return Ok(ToolOutput::error("Missing required field: file_path")),
        };

        let diff = match input.get("diff").and_then(Value::as_str) {
            Some(diff) => diff,
            None => return Ok(ToolOutput::error("Missing required field: diff")),
        };

        let resolved = self.resolve_path(file_path);
        let content = match std::fs::read_to_string(&resolved) {
            Ok(content) => content,
            Err(error) => {
                return Ok(ToolOutput::error(format!(
                    "Failed to read file {}: {}",
                    resolved.display(),
                    error
                )))
            }
        };

        match apply_patch(&content, diff) {
            Ok((patched, hunk_count)) => {
                if let Err(error) = std::fs::write(&resolved, patched) {
                    return Ok(ToolOutput::error(format!(
                        "Failed to write file {}: {}",
                        resolved.display(),
                        error
                    )));
                }
                Ok(ToolOutput::success(format!(
                    "Applied {hunk_count} hunk(s) to {}",
                    resolved.display()
                )))
            }
            Err(error) => Ok(ToolOutput::error(error)),
        }
    }
}

/// Parse and apply a unified diff to file content
fn apply_patch(content: &str, diff: &str) -> Result<(String, usize), String> {
    let hunks = parse_hunks(diff)?;
    if hunks.is_empty() {
        return Err("No valid hunks found in diff".to_string());
    }

    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let hunk_count = hunks.len();

    // Apply hunks in reverse order to preserve line numbers
    for hunk in hunks.iter().rev() {
        apply_hunk(&mut lines, hunk)?;
    }

    Ok((lines.join("\n") + "\n", hunk_count))
}

#[derive(Debug)]
struct Hunk {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
    lines: Vec<HunkLine>,
}

#[derive(Debug, Clone)]
enum HunkLine {
    Context(String),
    Remove(String),
    Add(String),
}

/// Parse unified diff format into hunks
fn parse_hunks(diff: &str) -> Result<Vec<Hunk>, String> {
    let mut hunks = Vec::new();
    let mut current_hunk: Option<Hunk> = None;

    for line in diff.lines() {
        if line.starts_with("@@") {
            // Save previous hunk if exists
            if let Some(hunk) = current_hunk.take() {
                hunks.push(hunk);
            }

            // Parse hunk header: @@ -old_start,old_count +new_start,new_count @@
            let header = parse_hunk_header(line)?;
            current_hunk = Some(Hunk {
                old_start: header.0,
                old_count: header.1,
                new_start: header.2,
                new_count: header.3,
                lines: Vec::new(),
            });
        } else if let Some(ref mut hunk) = current_hunk {
            // Parse hunk lines
            if line.starts_with(' ') {
                hunk.lines.push(HunkLine::Context(line[1..].to_string()));
            } else if line.starts_with('-') {
                hunk.lines.push(HunkLine::Remove(line[1..].to_string()));
            } else if line.starts_with('+') {
                hunk.lines.push(HunkLine::Add(line[1..].to_string()));
            }
            // Ignore other lines (file headers, etc.)
        }
    }

    // Save last hunk
    if let Some(hunk) = current_hunk {
        hunks.push(hunk);
    }

    Ok(hunks)
}

/// Parse hunk header line: @@ -old_start,old_count +new_start,new_count @@
fn parse_hunk_header(line: &str) -> Result<(usize, usize, usize, usize), String> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 3 || !parts[0].starts_with("@@") {
        return Err(format!("Invalid hunk header: {line}"));
    }

    let old_part = parts[1];
    let new_part = parts[2];

    let (old_start, old_count) = parse_range(old_part.trim_start_matches('-'))?;
    let (new_start, new_count) = parse_range(new_part.trim_start_matches('+'))?;

    Ok((old_start, old_count, new_start, new_count))
}

/// Parse range like "1,3" or "1" (count defaults to 1)
fn parse_range(range: &str) -> Result<(usize, usize), String> {
    let parts: Vec<&str> = range.split(',').collect();
    let start = parts[0]
        .parse::<usize>()
        .map_err(|_| format!("Invalid range start: {range}"))?;
    let count = if parts.len() > 1 {
        parts[1]
            .parse::<usize>()
            .map_err(|_| format!("Invalid range count: {range}"))?
    } else {
        1
    };
    Ok((start, count))
}

/// Apply a single hunk to the file lines
fn apply_hunk(lines: &mut Vec<String>, hunk: &Hunk) -> Result<(), String> {
    // Convert 1-indexed to 0-indexed
    let start_idx = if hunk.old_start > 0 {
        hunk.old_start - 1
    } else {
        0
    };

    // Verify context lines match
    let mut file_idx = start_idx;
    let mut hunk_idx = 0;

    // First pass: verify all context and remove lines match
    while hunk_idx < hunk.lines.len() {
        match &hunk.lines[hunk_idx] {
            HunkLine::Context(expected) | HunkLine::Remove(expected) => {
                if file_idx >= lines.len() {
                    return Err(format!(
                        "Hunk context mismatch: file has {} lines, expected more at line {}",
                        lines.len(),
                        file_idx + 1
                    ));
                }
                if &lines[file_idx] != expected {
                    return Err(format!(
                        "Hunk context mismatch at line {}: expected '{}', found '{}'",
                        file_idx + 1,
                        expected,
                        lines[file_idx]
                    ));
                }
                file_idx += 1;
            }
            HunkLine::Add(_) => {
                // Skip add lines in verification pass
            }
        }
        hunk_idx += 1;
    }

    // Second pass: apply changes
    let mut new_lines = Vec::new();
    let mut file_idx = start_idx;
    let mut hunk_idx = 0;

    // Copy lines before hunk
    new_lines.extend_from_slice(&lines[..start_idx]);

    // Apply hunk
    while hunk_idx < hunk.lines.len() {
        match &hunk.lines[hunk_idx] {
            HunkLine::Context(line) => {
                new_lines.push(line.clone());
                file_idx += 1;
            }
            HunkLine::Remove(_) => {
                // Skip removed line
                file_idx += 1;
            }
            HunkLine::Add(line) => {
                new_lines.push(line.clone());
            }
        }
        hunk_idx += 1;
    }

    // Copy remaining lines after hunk
    new_lines.extend_from_slice(&lines[file_idx..]);

    *lines = new_lines;
    Ok(())
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
        let dir = std::env::temp_dir().join(format!("openrust-patch-test-{unique}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[tokio::test]
    async fn test_patch_single_hunk() {
        let dir = create_temp_dir();
        let file = dir.join("test.txt");
        fs::write(&file, "line 1\nline 2\nline 3\n").expect("write file");

        let diff = r#"@@ -1,3 +1,3 @@
 line 1
-line 2
+line 2 modified
 line 3
"#;

        let tool = PatchTool::new(dir.clone());
        let output = tool
            .execute(json!({
                "file_path": "test.txt",
                "diff": diff
            }))
            .await
            .expect("execute");

        assert!(!output.is_error, "Output was error: {}", output.content);
        assert!(output.content.contains("Applied 1 hunk(s)"));

        let result = fs::read_to_string(file).expect("read");
        assert_eq!(result, "line 1\nline 2 modified\nline 3\n");
    }

    #[tokio::test]
    async fn test_patch_multi_hunk() {
        let dir = create_temp_dir();
        let file = dir.join("test.txt");
        fs::write(&file, "line 1\nline 2\nline 3\nline 4\n").expect("write file");

        let diff = r#"@@ -1,2 +1,2 @@
-line 1
+line 1 modified
 line 2
@@ -3,2 +3,2 @@
 line 3
-line 4
+line 4 modified
"#;

        let tool = PatchTool::new(dir.clone());
        let output = tool
            .execute(json!({
                "file_path": "test.txt",
                "diff": diff
            }))
            .await
            .expect("execute");

        assert!(!output.is_error, "Output was error: {}", output.content);
        assert!(output.content.contains("Applied 2 hunk(s)"));

        let result = fs::read_to_string(file).expect("read");
        assert_eq!(result, "line 1 modified\nline 2\nline 3\nline 4 modified\n");
    }

    #[tokio::test]
    async fn test_patch_context_mismatch() {
        let dir = create_temp_dir();
        let file = dir.join("test.txt");
        fs::write(&file, "line 1\nline 2\nline 3\n").expect("write file");

        let diff = r#"@@ -1,3 +1,3 @@
 line 1
-line 99
+line 2 modified
 line 3
"#;

        let tool = PatchTool::new(dir);
        let output = tool
            .execute(json!({
                "file_path": "test.txt",
                "diff": diff
            }))
            .await
            .expect("execute");

        assert!(output.is_error);
        assert!(output.content.contains("context mismatch"));
    }

    #[tokio::test]
    async fn test_patch_add_lines() {
        let dir = create_temp_dir();
        let file = dir.join("test.txt");
        fs::write(&file, "line 1\nline 3\n").expect("write file");

        let diff = r#"@@ -1,2 +1,3 @@
 line 1
+line 2
 line 3
"#;

        let tool = PatchTool::new(dir.clone());
        let output = tool
            .execute(json!({
                "file_path": "test.txt",
                "diff": diff
            }))
            .await
            .expect("execute");

        assert!(!output.is_error, "Output was error: {}", output.content);

        let result = fs::read_to_string(file).expect("read");
        assert_eq!(result, "line 1\nline 2\nline 3\n");
    }

    #[tokio::test]
    async fn test_patch_remove_lines() {
        let dir = create_temp_dir();
        let file = dir.join("test.txt");
        fs::write(&file, "line 1\nline 2\nline 3\n").expect("write file");

        let diff = r#"@@ -1,3 +1,2 @@
 line 1
-line 2
 line 3
"#;

        let tool = PatchTool::new(dir.clone());
        let output = tool
            .execute(json!({
                "file_path": "test.txt",
                "diff": diff
            }))
            .await
            .expect("execute");

        assert!(!output.is_error, "Output was error: {}", output.content);

        let result = fs::read_to_string(file).expect("read");
        assert_eq!(result, "line 1\nline 3\n");
    }
}
