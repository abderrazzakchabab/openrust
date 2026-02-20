use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

const DEFAULT_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}

pub struct ShellTool {
    working_dir: PathBuf,
}

impl ShellTool {
    pub fn new(working_dir: PathBuf) -> Self {
        ShellTool { working_dir }
    }

    pub async fn run(
        &self,
        command: &str,
        timeout_secs: Option<u64>,
    ) -> Result<CommandOutput> {
        let duration = Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS));

        let fut = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();

        let output = timeout(duration, fut)
            .await
            .with_context(|| format!("Command timed out after {} seconds", duration.as_secs()))?
            .with_context(|| format!("Failed to execute command: {}", command))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        Ok(CommandOutput {
            stdout,
            stderr,
            exit_code,
            success: output.status.success(),
        })
    }

    pub fn format_output(output: &CommandOutput) -> String {
        let mut result = String::new();
        if !output.stdout.is_empty() {
            result.push_str(&output.stdout);
        }
        if !output.stderr.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str("STDERR:\n");
            result.push_str(&output.stderr);
        }
        if result.is_empty() {
            result = format!("(exit code: {})", output.exit_code);
        }
        result
    }
}
