use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result};
use lsp_types::{
    ClientCapabilities, Diagnostic, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, InitializeParams, InitializeResult, InitializedParams,
    PublishDiagnosticsParams, ServerCapabilities, TextDocumentContentChangeEvent,
    TextDocumentIdentifier, TextDocumentItem, Uri, VersionedTextDocumentIdentifier,
};
use serde_json::Value;

use crate::lsp::transport::LspTransport;

pub struct LspClient {
    transport: Option<LspTransport>,
    server_capabilities: Option<ServerCapabilities>,
    diagnostics: HashMap<Uri, Vec<Diagnostic>>,
    root_uri: Uri,
}

impl LspClient {
    pub async fn start(command: &str, args: &[String], root_path: &PathBuf) -> Result<Self> {
        let mut transport = LspTransport::spawn(command, args).await?;
        let root_uri = path_to_uri(root_path)?;

        let init_params = InitializeParams {
            process_id: Some(std::process::id()),
            root_uri: Some(root_uri.clone()),
            capabilities: ClientCapabilities::default(),
            ..Default::default()
        };

        let result: InitializeResult = transport.send_request("initialize", init_params).await?;
        transport
            .send_notification("initialized", InitializedParams {})
            .await?;

        Ok(Self {
            transport: Some(transport),
            server_capabilities: Some(result.capabilities),
            diagnostics: HashMap::new(),
            root_uri,
        })
    }

    pub async fn open_file(
        &mut self,
        path: &PathBuf,
        content: &str,
        language_id: &str,
    ) -> Result<()> {
        let uri = path_to_uri(path)?;
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: language_id.to_string(),
                version: 1,
                text: content.to_string(),
            },
        };

        self.transport_mut()?
            .send_notification("textDocument/didOpen", params)
            .await
    }

    pub async fn close_file(&mut self, path: &PathBuf) -> Result<()> {
        let uri = path_to_uri(path)?;
        let params = DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
        };

        self.transport_mut()?
            .send_notification("textDocument/didClose", params)
            .await
    }

    pub async fn update_file(&mut self, path: &PathBuf, content: &str, version: i32) -> Result<()> {
        let uri = path_to_uri(path)?;
        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri, version },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: content.to_string(),
            }],
        };

        self.transport_mut()?
            .send_notification("textDocument/didChange", params)
            .await
    }

    pub async fn process_notifications(&mut self) -> Result<()> {
        let msg = self.transport_mut()?.read_message().await?;
        if msg.get("method").and_then(Value::as_str) == Some("textDocument/publishDiagnostics") {
            let params: PublishDiagnosticsParams =
                serde_json::from_value(msg.get("params").cloned().unwrap_or(Value::Null))
                    .context("Failed to parse publishDiagnostics params")?;
            self.store_diagnostics(params.uri, params.diagnostics);
        }

        Ok(())
    }

    pub fn store_diagnostics(&mut self, uri: Uri, diagnostics: Vec<Diagnostic>) {
        self.diagnostics.insert(uri, diagnostics);
    }

    pub fn get_diagnostics(&self, path: &PathBuf) -> Vec<&Diagnostic> {
        let Ok(uri) = path_to_uri(path) else {
            return Vec::new();
        };

        self.diagnostics
            .get(&uri)
            .map(|items| items.iter().collect())
            .unwrap_or_default()
    }

    pub fn get_all_diagnostics(&self) -> Vec<(String, &Vec<Diagnostic>)> {
        self.diagnostics
            .iter()
            .map(|(uri, diagnostics)| (uri.as_str().to_string(), diagnostics))
            .collect()
    }

    pub fn format_diagnostics(&self) -> String {
        if self.diagnostics.is_empty() {
            return "No diagnostics".to_string();
        }

        let mut lines = Vec::new();
        let mut entries: Vec<_> = self.diagnostics.iter().collect();
        entries.sort_by(|(a, _), (b, _)| a.as_str().cmp(b.as_str()));

        for (uri, diagnostics) in entries {
            lines.push(uri.as_str().to_string());
            if diagnostics.is_empty() {
                lines.push("  (no issues)".to_string());
                continue;
            }

            for diag in diagnostics {
                let severity = diag
                    .severity
                    .map(|s| format!("{:?}", s).to_lowercase())
                    .unwrap_or_else(|| "unknown".to_string());
                let line = diag.range.start.line + 1;
                let character = diag.range.start.character + 1;
                let source = diag
                    .source
                    .as_deref()
                    .map(|s| format!(" [{s}]"))
                    .unwrap_or_default();
                lines.push(format!(
                    "  - {severity} {line}:{character}{source}: {}",
                    diag.message
                ));
            }
        }

        lines.join("\n")
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(transport) = &mut self.transport {
            transport.shutdown().await?;
        }
        self.transport = None;
        Ok(())
    }

    pub fn root_uri(&self) -> &Uri {
        &self.root_uri
    }

    pub fn server_capabilities(&self) -> Option<&ServerCapabilities> {
        self.server_capabilities.as_ref()
    }

    pub fn language_id_for_path(path: &Path) -> &'static str {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("rs") => "rust",
            Some("py") => "python",
            Some("js") => "javascript",
            Some("jsx") => "javascriptreact",
            Some("ts") => "typescript",
            Some("tsx") => "typescriptreact",
            Some("go") => "go",
            Some("java") => "java",
            Some("c") => "c",
            Some("cpp") | Some("cc") | Some("cxx") => "cpp",
            Some("h") | Some("hpp") => "cpp",
            Some("json") => "json",
            Some("toml") => "toml",
            Some("yaml") | Some("yml") => "yaml",
            Some("md") => "markdown",
            _ => "plaintext",
        }
    }

    fn transport_mut(&mut self) -> Result<&mut LspTransport> {
        self.transport
            .as_mut()
            .context("LSP transport is not available")
    }
}

pub fn path_to_uri(path: &Path) -> Result<Uri> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .context("Failed to get current directory")?
            .join(path)
    };

    let path_str = absolute.to_string_lossy().replace(' ', "%20");
    let uri = if path_str.starts_with('/') {
        format!("file://{path_str}")
    } else {
        format!("file:///{path_str}")
    };

    Uri::from_str(&uri).with_context(|| format!("Invalid file URI: {uri}"))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use anyhow::Result;
    use lsp_types::{Diagnostic, ServerCapabilities};
    use serde_json::json;

    use super::{path_to_uri, LspClient};

    #[test]
    fn test_initialize_params_serialization() {
        let params = lsp_types::InitializeParams {
            process_id: Some(4242),
            root_uri: Some(path_to_uri(&PathBuf::from("/tmp/project")).expect("path to uri")),
            capabilities: lsp_types::ClientCapabilities::default(),
            ..Default::default()
        };

        let value = serde_json::to_value(params).expect("serialize initialize params");
        assert_eq!(value["processId"], 4242);
        assert!(value["rootUri"].as_str().is_some());
    }

    #[test]
    fn test_diagnostics_storage() -> Result<()> {
        let uri = path_to_uri(&PathBuf::from("/tmp/main.rs"))?;
        let diagnostic: Diagnostic = serde_json::from_value(json!({
            "range": {
                "start": {"line": 0, "character": 0},
                "end": {"line": 0, "character": 3}
            },
            "severity": 1,
            "message": "boom",
            "source": "rustc"
        }))?;

        let mut client = LspClient {
            transport: None,
            server_capabilities: Some(ServerCapabilities::default()),
            diagnostics: HashMap::new(),
            root_uri: path_to_uri(&PathBuf::from("/tmp"))?,
        };

        client.store_diagnostics(uri, vec![diagnostic]);
        let items = client.get_diagnostics(&PathBuf::from("/tmp/main.rs"));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].message, "boom");

        Ok(())
    }

    #[test]
    fn test_diagnostics_formatting() -> Result<()> {
        let uri = path_to_uri(&PathBuf::from("/tmp/lib.rs"))?;
        let diagnostic: Diagnostic = serde_json::from_value(json!({
            "range": {
                "start": {"line": 9, "character": 4},
                "end": {"line": 9, "character": 8}
            },
            "severity": 2,
            "message": "unused variable",
            "source": "rustc"
        }))?;

        let mut client = LspClient {
            transport: None,
            server_capabilities: Some(ServerCapabilities::default()),
            diagnostics: HashMap::new(),
            root_uri: path_to_uri(&PathBuf::from("/tmp"))?,
        };

        client.store_diagnostics(uri, vec![diagnostic]);
        let formatted = client.format_diagnostics();

        assert!(formatted.contains("unused variable"));
        assert!(formatted.contains("10:5"));
        assert!(formatted.contains("rustc"));

        Ok(())
    }

    #[test]
    fn test_file_url_conversion() -> Result<()> {
        let uri = path_to_uri(&PathBuf::from("/tmp/test.rs"))?;
        assert_eq!(uri.scheme().expect("uri scheme").as_str(), "file");
        assert!(uri.to_string().ends_with("/tmp/test.rs"));
        Ok(())
    }

    #[test]
    fn test_language_detection() {
        assert_eq!(
            LspClient::language_id_for_path(&PathBuf::from("src/main.rs")),
            "rust"
        );
        assert_eq!(
            LspClient::language_id_for_path(&PathBuf::from("notes.txt")),
            "plaintext"
        );
        assert_eq!(
            LspClient::language_id_for_path(&PathBuf::from("component.tsx")),
            "typescriptreact"
        );
    }
}
