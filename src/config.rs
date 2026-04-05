use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub provider: ProviderConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub tools: ToolsConfig,
    #[serde(default)]
    pub permissions: PermissionsConfig,
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
    #[serde(default)]
    pub custom_commands: HashMap<String, CustomCommandConfig>,
    #[serde(default)]
    pub custom_tools: HashMap<String, CustomToolConfig>,
    #[serde(default)]
    pub rules: RulesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    #[serde(default = "default_provider_name")]
    pub default: String,
    #[serde(default)]
    pub anthropic: AnthropicConfig,
    #[serde(default)]
    pub openai: OpenAIConfig,
    #[serde(default)]
    pub google: Option<GoogleProviderConfig>,
    #[serde(default)]
    pub bedrock: Option<BedrockConfig>,
    #[serde(default)]
    pub azure: Option<AzureConfig>,
    #[serde(default)]
    pub openrouter: Option<OpenRouterConfig>,
    #[serde(default)]
    pub ollama: Option<OllamaConfig>,
    #[serde(default)]
    pub custom: HashMap<String, CustomProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicConfig {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_anthropic_model")]
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_openai_model")]
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default)]
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleProviderConfig {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_google_model")]
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockConfig {
    #[serde(default)]
    pub access_key: Option<String>,
    #[serde(default)]
    pub secret_key: Option<String>,
    #[serde(default)]
    pub session_token: Option<String>,
    #[serde(default = "default_bedrock_region")]
    pub region: Option<String>,
    #[serde(default = "default_bedrock_model")]
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureConfig {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub deployment: String,
    #[serde(default = "default_azure_api_version")]
    pub api_version: Option<String>,
    #[serde(default = "default_azure_model")]
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterConfig {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_openrouter_model")]
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    #[serde(default = "default_ollama_base_url")]
    pub base_url: Option<String>,
    #[serde(default = "default_ollama_model")]
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProviderConfig {
    #[serde(default)]
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default)]
    pub extra_headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionsConfig {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub ask: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
    #[serde(default)]
    pub bash_allow_patterns: Vec<String>,
    #[serde(default)]
    pub bash_deny_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
    #[serde(default)]
    pub oauth: Option<crate::mcp::OAuthConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_true")]
    pub syntax_highlight: bool,
    #[serde(default = "default_true")]
    pub show_line_numbers: bool,
    #[serde(default = "default_max_history")]
    pub max_history: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    #[serde(default = "default_true")]
    pub allow_shell: bool,
    #[serde(default = "default_true")]
    pub allow_file_write: bool,
    #[serde(default = "default_true")]
    pub allow_git: bool,
    #[serde(default)]
    pub working_directory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomCommandConfig {
    pub description: String,
    pub prompt: String,
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomToolConfig {
    pub command: String,
    pub description: String,
    pub parameters: serde_json::Value,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub project_path: Option<String>,
    #[serde(default)]
    pub user_path: Option<String>,
    #[serde(default)]
    pub inline: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            provider: ProviderConfig::default(),
            ui: UiConfig::default(),
            tools: ToolsConfig::default(),
            permissions: PermissionsConfig::default(),
            mcp_servers: HashMap::new(),
            custom_commands: HashMap::new(),
            custom_tools: HashMap::new(),
            rules: RulesConfig::default(),
        }
    }
}

impl Default for RulesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            project_path: None,
            user_path: None,
            inline: None,
        }
    }
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            default: default_provider_name(),
            anthropic: AnthropicConfig::default(),
            openai: OpenAIConfig::default(),
            google: Some(GoogleProviderConfig::default()),
            bedrock: Some(BedrockConfig::default()),
            azure: Some(AzureConfig::default()),
            openrouter: Some(OpenRouterConfig::default()),
            ollama: Some(OllamaConfig::default()),
            custom: HashMap::new(),
        }
    }
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("ANTHROPIC_API_KEY").ok(),
            model: default_anthropic_model(),
            max_tokens: default_max_tokens(),
        }
    }
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("OPENAI_API_KEY").ok(),
            model: default_openai_model(),
            max_tokens: default_max_tokens(),
            base_url: None,
        }
    }
}

impl Default for GoogleProviderConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("GOOGLE_API_KEY")
                .ok()
                .or_else(|| std::env::var("GEMINI_API_KEY").ok()),
            model: default_google_model(),
            max_tokens: default_max_tokens(),
        }
    }
}

impl Default for BedrockConfig {
    fn default() -> Self {
        Self {
            access_key: std::env::var("AWS_ACCESS_KEY_ID").ok(),
            secret_key: std::env::var("AWS_SECRET_ACCESS_KEY").ok(),
            session_token: std::env::var("AWS_SESSION_TOKEN").ok(),
            region: default_bedrock_region(),
            model: default_bedrock_model(),
            max_tokens: default_max_tokens(),
        }
    }
}

impl Default for AzureConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("AZURE_OPENAI_API_KEY").ok(),
            endpoint: String::new(),
            deployment: String::new(),
            api_version: default_azure_api_version(),
            model: default_azure_model(),
            max_tokens: default_max_tokens(),
        }
    }
}

impl Default for OpenRouterConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("OPENROUTER_API_KEY").ok(),
            model: default_openrouter_model(),
            max_tokens: default_max_tokens(),
        }
    }
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: default_ollama_base_url(),
            model: default_ollama_model(),
            max_tokens: default_max_tokens(),
        }
    }
}

impl Default for PermissionsConfig {
    fn default() -> Self {
        Self {
            allow: Vec::new(),
            ask: Vec::new(),
            deny: Vec::new(),
            bash_allow_patterns: Vec::new(),
            bash_deny_patterns: Vec::new(),
        }
    }
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            command: None,
            args: None,
            env: None,
            url: None,
            headers: None,
            oauth: None,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            syntax_highlight: true,
            show_line_numbers: true,
            max_history: default_max_history(),
        }
    }
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            allow_shell: true,
            allow_file_write: true,
            allow_git: true,
            working_directory: None,
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
            let mut config = Config::default();
            config.apply_env_overrides();
            config.save()?;
            return Ok(config);
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config from {}", path.display()))?;

        let mut config: Config =
            toml::from_str(&content).with_context(|| "Failed to parse config file")?;

        config.apply_env_overrides();

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create config directory {}", dir.display()))?;

        let content = toml::to_string_pretty(self).with_context(|| "Failed to serialize config")?;

        std::fs::write(Self::config_path(), content)
            .with_context(|| "Failed to write config file")?;

        Ok(())
    }

    pub fn get_active_api_key(&self) -> Option<&str> {
        match self.provider.default.as_str() {
            "anthropic" => self.provider.anthropic.api_key.as_deref(),
            "openai" => self.provider.openai.api_key.as_deref(),
            "google" | "gemini" => self
                .provider
                .google
                .as_ref()
                .and_then(|cfg| cfg.api_key.as_deref()),
            "bedrock" | "aws-bedrock" => self
                .provider
                .bedrock
                .as_ref()
                .and_then(|cfg| cfg.access_key.as_deref()),
            "azure" | "azure-openai" => self
                .provider
                .azure
                .as_ref()
                .and_then(|cfg| cfg.api_key.as_deref()),
            "openrouter" => self
                .provider
                .openrouter
                .as_ref()
                .and_then(|cfg| cfg.api_key.as_deref()),
            "ollama" => None,
            custom_name => self
                .provider
                .custom
                .get(custom_name)
                .and_then(|cfg| cfg.api_key.as_deref()),
        }
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            self.provider.anthropic.api_key = Some(key);
        }

        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            self.provider.openai.api_key = Some(key);
        }

        if let Ok(key) = std::env::var("GOOGLE_API_KEY") {
            let mut google = self.provider.google.clone().unwrap_or_default();
            google.api_key = Some(key);
            self.provider.google = Some(google);
        } else if let Ok(key) = std::env::var("GEMINI_API_KEY") {
            let mut google = self.provider.google.clone().unwrap_or_default();
            google.api_key = Some(key);
            self.provider.google = Some(google);
        }

        if let Ok(key) = std::env::var("AWS_ACCESS_KEY_ID") {
            let mut bedrock = self.provider.bedrock.clone().unwrap_or_default();
            bedrock.access_key = Some(key);
            self.provider.bedrock = Some(bedrock);
        }

        if let Ok(key) = std::env::var("AWS_SECRET_ACCESS_KEY") {
            let mut bedrock = self.provider.bedrock.clone().unwrap_or_default();
            bedrock.secret_key = Some(key);
            self.provider.bedrock = Some(bedrock);
        }

        if let Ok(key) = std::env::var("AWS_SESSION_TOKEN") {
            let mut bedrock = self.provider.bedrock.clone().unwrap_or_default();
            bedrock.session_token = Some(key);
            self.provider.bedrock = Some(bedrock);
        }

        if let Ok(key) = std::env::var("AZURE_OPENAI_API_KEY") {
            let mut azure = self.provider.azure.clone().unwrap_or_default();
            azure.api_key = Some(key);
            self.provider.azure = Some(azure);
        }

        if let Ok(key) = std::env::var("OPENROUTER_API_KEY") {
            let mut openrouter = self.provider.openrouter.clone().unwrap_or_default();
            openrouter.api_key = Some(key);
            self.provider.openrouter = Some(openrouter);
        }
    }
}

fn default_provider_name() -> String {
    "anthropic".to_string()
}

fn default_true() -> bool {
    true
}

fn default_max_tokens() -> u32 {
    8192
}

fn default_max_history() -> usize {
    1000
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_anthropic_model() -> String {
    "claude-sonnet-4-20250514".to_string()
}

fn default_openai_model() -> String {
    "gpt-4o".to_string()
}

fn default_google_model() -> String {
    "gemini-2.0-flash-exp".to_string()
}

fn default_bedrock_region() -> Option<String> {
    Some("us-east-1".to_string())
}

fn default_bedrock_model() -> String {
    "anthropic.claude-v2".to_string()
}

fn default_azure_api_version() -> Option<String> {
    Some("2023-05-15".to_string())
}

fn default_azure_model() -> String {
    "gpt-4".to_string()
}

fn default_openrouter_model() -> String {
    "anthropic/claude-opus-4".to_string()
}

fn default_ollama_base_url() -> Option<String> {
    Some("http://localhost:11434/v1/chat/completions".to_string())
}

fn default_ollama_model() -> String {
    "llama2".to_string()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Config, CustomToolConfig};

    #[test]
    fn test_config_defaults_include_custom_tools_and_rules() {
        let config = Config::default();
        assert!(config.custom_tools.is_empty());
        assert!(config.rules.enabled);
    }

    #[test]
    fn test_config_toml_roundtrip_with_custom_tool() {
        let mut config = Config::default();
        config.custom_tools.insert(
            "my-tool".to_string(),
            CustomToolConfig {
                command: "python script.py".to_string(),
                description: "custom test tool".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "value": { "type": "string" }
                    }
                }),
                enabled: true,
            },
        );
        config.rules.inline = Some("be concise".to_string());

        let toml = toml::to_string_pretty(&config).expect("serialize config");
        let parsed: Config = toml::from_str(&toml).expect("parse config");

        assert!(parsed.custom_tools.contains_key("my-tool"));
        assert_eq!(parsed.rules.inline.as_deref(), Some("be concise"));
    }
}
