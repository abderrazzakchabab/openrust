use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub provider: ProviderConfig,
    pub ui: UiConfig,
    pub tools: ToolsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub default: String,
    pub anthropic: AnthropicConfig,
    pub openai: OpenAIConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicConfig {
    pub api_key: Option<String>,
    pub model: String,
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    pub api_key: Option<String>,
    pub model: String,
    pub max_tokens: u32,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub theme: String,
    pub syntax_highlight: bool,
    pub show_line_numbers: bool,
    pub max_history: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    pub allow_shell: bool,
    pub allow_file_write: bool,
    pub allow_git: bool,
    pub working_directory: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            provider: ProviderConfig {
                default: "anthropic".to_string(),
                anthropic: AnthropicConfig {
                    api_key: std::env::var("ANTHROPIC_API_KEY").ok(),
                    model: "claude-opus-4-6".to_string(),
                    max_tokens: 8192,
                },
                openai: OpenAIConfig {
                    api_key: std::env::var("OPENAI_API_KEY").ok(),
                    model: "gpt-4o".to_string(),
                    max_tokens: 8192,
                    base_url: None,
                },
            },
            ui: UiConfig {
                theme: "dark".to_string(),
                syntax_highlight: true,
                show_line_numbers: true,
                max_history: 1000,
            },
            tools: ToolsConfig {
                allow_shell: true,
                allow_file_write: true,
                allow_git: true,
                working_directory: None,
            },
        }
    }
}

impl Config {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("openrust")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if !path.exists() {
            let config = Config::default();
            config.save()?;
            return Ok(config);
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config from {}", path.display()))?;

        let mut config: Config = toml::from_str(&content)
            .with_context(|| "Failed to parse config file")?;

        // Override with environment variables
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            config.provider.anthropic.api_key = Some(key);
        }
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            config.provider.openai.api_key = Some(key);
        }

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create config directory {}", dir.display()))?;

        let content = toml::to_string_pretty(self)
            .with_context(|| "Failed to serialize config")?;

        std::fs::write(Self::config_path(), content)
            .with_context(|| "Failed to write config file")?;

        Ok(())
    }

    pub fn get_active_api_key(&self) -> Option<&str> {
        match self.provider.default.as_str() {
            "anthropic" => self.provider.anthropic.api_key.as_deref(),
            "openai" => self.provider.openai.api_key.as_deref(),
            _ => None,
        }
    }
}
