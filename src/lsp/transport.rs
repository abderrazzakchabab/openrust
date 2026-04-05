use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter,
};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

pub struct LspTransport {
    child: Child,
    writer: BufWriter<ChildStdin>,
    reader: BufReader<ChildStdout>,
    next_id: i64,
}

impl LspTransport {
    pub async fn spawn(command: &str, args: &[String]) -> Result<Self> {
        if command.trim().is_empty() {
            anyhow::bail!("LSP command cannot be empty");
        }

        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit());

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn LSP server: {command}"))?;

        let stdin = child
            .stdin
            .take()
            .context("Failed to capture LSP server stdin")?;
        let stdout = child
            .stdout
            .take()
            .context("Failed to capture LSP server stdout")?;

        Ok(Self {
            child,
            writer: BufWriter::new(stdin),
            reader: BufReader::new(stdout),
            next_id: 1,
        })
    }

    pub async fn send_request<P: Serialize, R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: P,
    ) -> Result<R> {
        self.ensure_running().await?;

        let id = self.next_id;
        self.next_id += 1;

        let payload = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let body = serde_json::to_string(&payload)
            .with_context(|| format!("Failed to serialize LSP request {method}"))?;
        Self::write_message(&mut self.writer, &body).await?;

        loop {
            let msg = self.read_message().await?;
            let msg_id = msg.get("id").and_then(Value::as_i64);

            if msg_id != Some(id) {
                continue;
            }

            if let Some(err) = msg.get("error") {
                let code = err.get("code").and_then(Value::as_i64).unwrap_or_default();
                let message = err
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("Unknown LSP error");
                return Err(anyhow!("LSP request {method} failed ({code}): {message}"));
            }

            let result = msg
                .get("result")
                .cloned()
                .ok_or_else(|| anyhow!("LSP response for {method} missing result"))?;

            return serde_json::from_value(result)
                .with_context(|| format!("Invalid LSP response payload for {method}"));
        }
    }

    pub async fn send_notification<P: Serialize>(&mut self, method: &str, params: P) -> Result<()> {
        self.ensure_running().await?;

        let payload = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        let body = serde_json::to_string(&payload)
            .with_context(|| format!("Failed to serialize LSP notification {method}"))?;
        Self::write_message(&mut self.writer, &body).await
    }

    pub async fn read_message(&mut self) -> Result<Value> {
        self.ensure_running().await?;

        let body = Self::read_content_length_message(&mut self.reader).await?;
        serde_json::from_str(&body).context("Failed to parse LSP JSON message")
    }

    async fn read_content_length_message(reader: &mut BufReader<ChildStdout>) -> Result<String> {
        read_content_length_message_impl(reader).await
    }

    async fn write_message(writer: &mut BufWriter<ChildStdin>, body: &str) -> Result<()> {
        let framed = frame_message(body);
        writer
            .write_all(&framed)
            .await
            .context("Failed to write framed LSP message")?;
        writer
            .flush()
            .await
            .context("Failed to flush framed LSP message")
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        if self
            .child
            .try_wait()
            .context("Failed to check LSP process status")?
            .is_some()
        {
            return Ok(());
        }

        let _ = self.send_request::<_, Value>("shutdown", json!({})).await;
        let _ = self.send_notification("exit", json!({})).await;

        match tokio::time::timeout(Duration::from_secs(2), self.child.wait()).await {
            Ok(wait_result) => {
                wait_result.context("Failed waiting for LSP server to exit")?;
            }
            Err(_) => {
                self.child
                    .kill()
                    .await
                    .context("Failed to kill LSP server process")?;
            }
        }

        Ok(())
    }

    async fn ensure_running(&mut self) -> Result<()> {
        if let Some(status) = self
            .child
            .try_wait()
            .context("Failed to check LSP process status")?
        {
            anyhow::bail!("LSP server process is not running: {status}");
        }

        Ok(())
    }
}

impl Drop for LspTransport {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.start_kill();
        }
    }
}

fn frame_message(body: &str) -> Vec<u8> {
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut out = Vec::with_capacity(header.len() + body.len());
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(body.as_bytes());
    out
}

fn parse_content_length(headers: &[String]) -> Result<usize> {
    for line in headers {
        if let Some(value) = line.strip_prefix("Content-Length:") {
            return value
                .trim()
                .parse::<usize>()
                .context("Invalid Content-Length value");
        }
    }

    Err(anyhow!("Missing Content-Length header"))
}

async fn read_content_length_message_impl<R>(reader: &mut R) -> Result<String>
where
    R: AsyncBufRead + AsyncRead + Unpin,
{
    let mut headers = Vec::new();

    loop {
        let mut line = String::new();
        let bytes = reader
            .read_line(&mut line)
            .await
            .context("Failed to read LSP header line")?;

        if bytes == 0 {
            return Err(anyhow!("LSP stream closed while reading headers"));
        }

        if line == "\r\n" || line == "\n" {
            break;
        }

        headers.push(line.trim_end_matches(['\r', '\n']).to_string());
    }

    let len = parse_content_length(&headers)?;
    let mut body = vec![0_u8; len];
    reader
        .read_exact(&mut body)
        .await
        .context("Failed to read LSP message body")?;

    String::from_utf8(body).context("LSP message body was not valid UTF-8")
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use tokio::io::BufReader;

    use super::{frame_message, parse_content_length, read_content_length_message_impl};

    #[test]
    fn test_content_length_framing() {
        let body = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
        let framed = frame_message(body);
        let framed_str = String::from_utf8(framed).expect("framed message is valid UTF-8");
        assert_eq!(
            framed_str,
            format!("Content-Length: {}\r\n\r\n{body}", body.len())
        );
    }

    #[test]
    fn test_parse_content_length() {
        let headers = vec![
            "Content-Type: application/vscode-jsonrpc; charset=utf-8".to_string(),
            "Content-Length: 42".to_string(),
        ];

        let len = parse_content_length(&headers).expect("parse content length");
        assert_eq!(len, 42);
    }

    #[tokio::test]
    async fn test_read_content_length_message() -> Result<()> {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":{}}"#;
        let payload = format!("Content-Length: {}\r\n\r\n{body}", body.len());
        let mut reader = BufReader::new(payload.as_bytes());

        let parsed = read_content_length_message_impl(&mut reader).await?;
        assert_eq!(parsed, body);

        Ok(())
    }
}
