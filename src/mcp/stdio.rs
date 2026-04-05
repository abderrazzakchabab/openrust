use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use crate::mcp::protocol::{JsonRpcRequest, JsonRpcResponse};

pub struct StdioTransport {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: AtomicU64,
}

impl StdioTransport {
    pub async fn spawn(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<Self> {
        if command.trim().is_empty() {
            anyhow::bail!("MCP command cannot be empty");
        }

        let mut cmd = Command::new(command);
        cmd.args(args)
            .envs(env)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit());

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn MCP server: {command}"))?;

        let stdin = child
            .stdin
            .take()
            .context("Failed to capture MCP server stdin")?;
        let stdout = child
            .stdout
            .take()
            .context("Failed to capture MCP server stdout")?;

        Ok(Self {
            child,
            stdin: BufWriter::new(stdin),
            stdout: BufReader::new(stdout),
            next_id: AtomicU64::new(1),
        })
    }

    pub async fn send_request(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<JsonRpcResponse> {
        self.ensure_running().await?;

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let serialized =
            serde_json::to_string(&req).context("Failed to serialize JSON-RPC request")?;
        self.stdin
            .write_all(serialized.as_bytes())
            .await
            .with_context(|| format!("Failed to write request for method {method}"))?;
        self.stdin
            .write_all(b"\n")
            .await
            .with_context(|| format!("Failed to write request delimiter for method {method}"))?;
        self.stdin
            .flush()
            .await
            .with_context(|| format!("Failed to flush request for method {method}"))?;

        let mut line = String::new();
        loop {
            line.clear();
            let bytes = self
                .stdout
                .read_line(&mut line)
                .await
                .with_context(|| format!("Failed to read response for method {method}"))?;

            if bytes == 0 {
                if let Some(status) = self
                    .child
                    .try_wait()
                    .context("Failed to read MCP process status")?
                {
                    return Err(anyhow!(
                        "MCP server exited while waiting for response to {method}: {status}"
                    ));
                }
                return Err(anyhow!(
                    "MCP server stdout closed while waiting for response to {method}"
                ));
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let value: Value = serde_json::from_str(trimmed)
                .with_context(|| format!("Invalid JSON received from MCP server: {trimmed}"))?;

            if value.get("method").is_some() && value.get("id").is_none() {
                continue;
            }

            let response: JsonRpcResponse = serde_json::from_value(value)
                .with_context(|| format!("Invalid JSON-RPC response for method {method}"))?;

            if response.id == Some(id) {
                return Ok(response);
            }
        }
    }

    pub async fn send_notification(&mut self, method: &str, params: Option<Value>) -> Result<()> {
        self.ensure_running().await?;

        let payload = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        let serialized =
            serde_json::to_string(&payload).context("Failed to serialize JSON-RPC notification")?;

        self.stdin
            .write_all(serialized.as_bytes())
            .await
            .with_context(|| format!("Failed to write notification for method {method}"))?;
        self.stdin.write_all(b"\n").await.with_context(|| {
            format!("Failed to write notification delimiter for method {method}")
        })?;
        self.stdin
            .flush()
            .await
            .with_context(|| format!("Failed to flush notification for method {method}"))?;

        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        if self
            .child
            .try_wait()
            .context("Failed to check MCP process status")?
            .is_some()
        {
            return Ok(());
        }

        let _ = self.send_request("shutdown", None).await;
        let _ = self.send_notification("exit", None).await;

        match tokio::time::timeout(std::time::Duration::from_secs(2), self.child.wait()).await {
            Ok(wait_result) => {
                wait_result.context("Failed waiting for MCP server to exit")?;
            }
            Err(_) => {
                self.child
                    .kill()
                    .await
                    .context("Failed to kill MCP server process")?;
            }
        }

        Ok(())
    }

    async fn ensure_running(&mut self) -> Result<()> {
        if let Some(status) = self
            .child
            .try_wait()
            .context("Failed to check MCP server process status")?
        {
            anyhow::bail!("MCP server process is not running: {status}");
        }

        Ok(())
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.start_kill();
        }
    }
}
