use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::config::CustomToolConfig;

use super::traits::{Tool, ToolOutput};

pub struct CustomTool {
    name: String,
    config: CustomToolConfig,
    working_dir: PathBuf,
}

impl CustomTool {
    pub fn new(name: String, config: CustomToolConfig, working_dir: PathBuf) -> Self {
        Self {
            name,
            config,
            working_dir,
        }
    }
}

#[async_trait]
impl Tool for CustomTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.config.description
    }

    fn parameters(&self) -> Value {
        self.config.parameters.clone()
    }

    async fn execute(&self, input: Value) -> anyhow::Result<ToolOutput> {
        let input_json = serde_json::to_vec(&input)?;

        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&self.config.command)
            .current_dir(&self.working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&input_json).await?;
        }

        let output = child.wait_with_output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if output.status.success() {
            let content = if stdout.is_empty() {
                "(no output)".to_string()
            } else {
                stdout
            };
            Ok(ToolOutput::success(content))
        } else {
            let content = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("Custom tool '{}' failed", self.name)
            };
            Ok(ToolOutput::error(content))
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
        let dir = std::env::temp_dir().join(format!("openrust-custom-tool-test-{unique}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn make_tool(command: &str) -> CustomTool {
        CustomTool::new(
            "my-tool".to_string(),
            CustomToolConfig {
                command: command.to_string(),
                description: "custom description".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "value": { "type": "string" }
                    }
                }),
                enabled: true,
            },
            create_temp_dir(),
        )
    }

    #[test]
    fn custom_tool_metadata() {
        let tool = make_tool("cat");
        assert_eq!(tool.name(), "my-tool");
        assert_eq!(tool.description(), "custom description");
        assert_eq!(tool.parameters()["type"], "object");
    }

    #[tokio::test]
    async fn custom_tool_execute_success() {
        let tool = make_tool("cat");
        let output = tool.execute(json!({"value": "hello"})).await.expect("exec");
        assert!(!output.is_error);
        assert!(output.content.contains("hello"));
    }

    #[tokio::test]
    async fn custom_tool_execute_failure() {
        let tool = make_tool("exit 2");
        let output = tool.execute(json!({"value": "hello"})).await.expect("exec");
        assert!(output.is_error);
    }
}
