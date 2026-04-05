pub mod client;
pub mod servers;
pub mod transport;

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;

pub struct LspManager {
    clients: HashMap<String, client::LspClient>,
}

impl LspManager {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    pub async fn start_server(
        &mut self,
        language_id: &str,
        command: &str,
        args: &[String],
        root: &PathBuf,
    ) -> Result<()> {
        let client = client::LspClient::start(command, args, root).await?;
        self.clients.insert(language_id.to_string(), client);
        Ok(())
    }

    pub async fn stop_server(&mut self, language_id: &str) -> Result<()> {
        if let Some(mut client) = self.clients.remove(language_id) {
            client.shutdown().await?;
        }
        Ok(())
    }

    pub async fn stop_all(&mut self) -> Result<()> {
        let languages: Vec<String> = self.clients.keys().cloned().collect();
        for language in languages {
            self.stop_server(&language).await?;
        }
        Ok(())
    }

    pub fn get_client(&self, language_id: &str) -> Option<&client::LspClient> {
        self.clients.get(language_id)
    }

    pub fn get_client_mut(&mut self, language_id: &str) -> Option<&mut client::LspClient> {
        self.clients.get_mut(language_id)
    }

    pub fn get_all_diagnostics(&self) -> String {
        if self.clients.is_empty() {
            return "No LSP servers running".to_string();
        }

        let mut lines = Vec::new();
        let mut entries: Vec<_> = self.clients.iter().collect();
        entries.sort_by(|(a, _), (b, _)| a.cmp(b));

        for (language, client) in entries {
            lines.push(format!("[{language}]"));
            lines.push(client.format_diagnostics());
        }

        lines.join("\n")
    }

    pub async fn auto_detect_and_start(&mut self, root_dir: &PathBuf) -> Result<Vec<String>> {
        let detected = servers::detect_servers(root_dir);
        let mut started = Vec::new();

        for server_def in detected {
            if servers::is_server_available(server_def.command) {
                let args: Vec<String> = server_def
                    .args
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                if self
                    .start_server(server_def.language_id, server_def.command, &args, root_dir)
                    .await
                    .is_ok()
                {
                    started.push(server_def.language_id.to_string());
                }
            }
        }

        Ok(started)
    }
}

impl Default for LspManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::LspManager;

    #[test]
    fn test_lsp_manager_new() {
        let manager = LspManager::new();
        assert!(manager.clients.is_empty());
    }

    #[test]
    fn test_lsp_manager_default() {
        let manager = LspManager::default();
        assert!(manager.clients.is_empty());
    }

    #[tokio::test]
    async fn test_auto_detect_and_start_empty_dir() {
        let mut manager = LspManager::new();
        let temp_dir = std::env::temp_dir();
        let result = manager.auto_detect_and_start(&temp_dir).await;
        assert!(result.is_ok());
    }
}
