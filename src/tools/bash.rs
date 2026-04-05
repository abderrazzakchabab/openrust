use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::process::Command;
use tokio::time::timeout;

use super::traits::{Tool, ToolOutput};

const DEFAULT_TIMEOUT_SECS: u64 = 120;
const MAX_OUTPUT_BYTES: usize = 100_000;

pub struct BashTool {
    working_dir: PathBuf,
}

impl BashTool {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }

    fn format_output(output: std::process::Output) -> String {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let mut result = String::new();

        if !stdout.is_empty() {
            result.push_str(&stdout);
        }

        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str("STDERR:\n");
            result.push_str(&stderr);
        }

        if !output.status.success() {
            if !result.is_empty() {
                result.push('\n');
            }
            let exit_code = output.status.code().unwrap_or(-1);
            result.push_str(&format!("(exit code: {exit_code})"));
        }

        if result.is_empty() {
            "(no output)".to_string()
        } else {
            Self::truncate_output(result)
        }
    }

    fn truncate_output(output: String) -> String {
        if output.len() <= MAX_OUTPUT_BYTES {
            return output;
        }

        let truncated = String::from_utf8_lossy(&output.as_bytes()[..MAX_OUTPUT_BYTES]).to_string();
        format!("{truncated}\n[output truncated]")
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a bash command in the configured working directory"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds (default 120)"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, input: Value) -> anyhow::Result<ToolOutput> {
        let command = match input.get("command").and_then(Value::as_str) {
            Some(command) => command,
            None => return Ok(ToolOutput::error("Missing required field: command")),
        };

        let timeout_secs = input
            .get("timeout")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_TIMEOUT_SECS);

        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg(command)
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = match timeout(Duration::from_secs(timeout_secs), cmd.output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => return Ok(ToolOutput::error(error.to_string())),
            Err(_) => {
                return Ok(ToolOutput::error(format!(
                    "Command timed out after {timeout_secs} seconds"
                )));
            }
        };

        Ok(ToolOutput::success(Self::format_output(output)))
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
        let dir = std::env::temp_dir().join(format!("openrust-bash-test-{unique}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[tokio::test]
    async fn test_bash_echo() {
        let tool = BashTool::new(create_temp_dir());
        let output = tool
            .execute(json!({"command": "echo hello"}))
            .await
            .expect("execute");

        assert!(!output.is_error);
        assert!(output.content.contains("hello"));
    }

    #[tokio::test]
    async fn test_bash_working_dir() {
        let working_dir = create_temp_dir();
        let tool = BashTool::new(working_dir.clone());

        let output = tool
            .execute(json!({"command": "pwd"}))
            .await
            .expect("execute");

        assert!(!output.is_error);
        assert!(output
            .content
            .contains(working_dir.to_string_lossy().as_ref()));
    }

    #[tokio::test]
    async fn test_bash_timeout() {
        let tool = BashTool::new(create_temp_dir());

        let output = tool
            .execute(json!({"command": "sleep 10", "timeout": 1}))
            .await
            .expect("execute");

        assert!(output.is_error);
        assert!(output.content.contains("timed out"));
    }

    #[tokio::test]
    async fn test_bash_stderr() {
        let tool = BashTool::new(create_temp_dir());

        let output = tool
            .execute(json!({"command": "echo err >&2"}))
            .await
            .expect("execute");

        assert!(!output.is_error);
        assert!(output.content.contains("STDERR:"));
        assert!(output.content.contains("err"));
    }
}
