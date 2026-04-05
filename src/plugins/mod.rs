use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            command: String::new(),
            args: Vec::new(),
            description: String::new(),
            enabled: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginMessage {
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginResponse {
    pub result: Option<Value>,
    pub error: Option<String>,
}

pub struct PluginManager {
    plugins: HashMap<String, PluginProcess>,
}

struct PluginProcess {
    config: PluginConfig,
    child: Option<Child>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: String, config: PluginConfig) {
        self.plugins.insert(
            name,
            PluginProcess {
                config,
                child: None,
            },
        );
    }

    pub fn start(&mut self, name: &str) -> Result<()> {
        let process = self
            .plugins
            .get_mut(name)
            .context("Plugin not registered")?;

        if process.child.is_some() {
            return Ok(());
        }

        if !process.config.enabled {
            anyhow::bail!("Plugin '{}' is disabled", name);
        }

        let mut cmd = Command::new(&process.config.command);
        cmd.args(&process.config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn plugin: {}", name))?;

        process.child = Some(child);
        Ok(())
    }

    pub fn send_message(&mut self, name: &str, message: PluginMessage) -> Result<PluginResponse> {
        let process = self
            .plugins
            .get_mut(name)
            .context("Plugin not registered")?;

        let child = process
            .child
            .as_mut()
            .context("Plugin process not started")?;

        let stdin = child.stdin.as_mut().context("Failed to get stdin")?;
        let stdout = child.stdout.as_mut().context("Failed to get stdout")?;

        let json_msg =
            serde_json::to_string(&message).context("Failed to serialize plugin message")?;

        writeln!(stdin, "{}", json_msg)
            .with_context(|| format!("Failed to write to plugin '{}'", name))?;
        stdin
            .flush()
            .with_context(|| format!("Failed to flush plugin '{}' stdin", name))?;

        let mut reader = BufReader::new(stdout);
        let mut response_line = String::new();
        reader
            .read_line(&mut response_line)
            .with_context(|| format!("Failed to read from plugin '{}'", name))?;

        let response: PluginResponse = serde_json::from_str(&response_line)
            .with_context(|| format!("Failed to parse plugin '{}' response", name))?;

        Ok(response)
    }

    pub fn stop(&mut self, name: &str) -> Result<()> {
        let process = self
            .plugins
            .get_mut(name)
            .context("Plugin not registered")?;

        if let Some(mut child) = process.child.take() {
            child
                .kill()
                .with_context(|| format!("Failed to kill plugin '{}'", name))?;
            child
                .wait()
                .with_context(|| format!("Failed to wait for plugin '{}'", name))?;
        }

        Ok(())
    }

    pub fn stop_all(&mut self) {
        let names: Vec<String> = self.plugins.keys().cloned().collect();
        for name in names {
            let _ = self.stop(&name);
        }
    }

    pub fn list(&self) -> Vec<(&str, &PluginConfig)> {
        self.plugins
            .iter()
            .map(|(name, process)| (name.as_str(), &process.config))
            .collect()
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PluginManager {
    fn drop(&mut self) {
        self.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_manager_new() {
        let manager = PluginManager::new();
        assert_eq!(manager.list().len(), 0);
    }

    #[test]
    fn test_plugin_register() {
        let mut manager = PluginManager::new();
        let config = PluginConfig {
            command: "echo".to_string(),
            args: vec!["test".to_string()],
            description: "Test plugin".to_string(),
            enabled: true,
        };

        manager.register("test".to_string(), config.clone());
        let list = manager.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].0, "test");
        assert_eq!(list[0].1.command, "echo");
    }

    #[test]
    fn test_plugin_list() {
        let mut manager = PluginManager::new();

        let config1 = PluginConfig {
            command: "echo".to_string(),
            args: vec![],
            description: "Plugin 1".to_string(),
            enabled: true,
        };
        let config2 = PluginConfig {
            command: "cat".to_string(),
            args: vec![],
            description: "Plugin 2".to_string(),
            enabled: false,
        };

        manager.register("plugin1".to_string(), config1);
        manager.register("plugin2".to_string(), config2);

        let list = manager.list();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_plugin_config_default() {
        let config = PluginConfig::default();
        assert_eq!(config.command, "");
        assert_eq!(config.args.len(), 0);
        assert_eq!(config.description, "");
        assert!(config.enabled);
    }

    #[test]
    fn test_plugin_config_serialization() {
        let config = PluginConfig {
            command: "test".to_string(),
            args: vec!["arg1".to_string()],
            description: "Test".to_string(),
            enabled: true,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: PluginConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.command, "test");
        assert_eq!(deserialized.args.len(), 1);
        assert_eq!(deserialized.description, "Test");
        assert!(deserialized.enabled);
    }

    #[test]
    fn test_plugin_message_serialization() {
        let message = PluginMessage {
            method: "execute".to_string(),
            params: serde_json::json!({"key": "value"}),
        };

        let json = serde_json::to_string(&message).unwrap();
        let deserialized: PluginMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.method, "execute");
        assert_eq!(
            deserialized.params.get("key").unwrap().as_str().unwrap(),
            "value"
        );
    }

    #[test]
    fn test_plugin_response_with_result() {
        let response = PluginResponse {
            result: Some(serde_json::json!({"status": "ok"})),
            error: None,
        };

        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_plugin_response_with_error() {
        let response = PluginResponse {
            result: None,
            error: Some("Plugin failed".to_string()),
        };

        assert!(response.result.is_none());
        assert_eq!(response.error.unwrap(), "Plugin failed");
    }
}
