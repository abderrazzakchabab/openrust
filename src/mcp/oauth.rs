use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub token_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_url: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
}

pub struct OAuthManager {
    tokens: HashMap<String, OAuthToken>,
    token_file: PathBuf,
    client: Client,
}

impl OAuthManager {
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_dir()
            .with_context(|| "Failed to determine data directory")?
            .join("openrust");
        std::fs::create_dir_all(&data_dir)
            .with_context(|| format!("Failed to create data directory: {:?}", data_dir))?;

        let token_file = data_dir.join("mcp_tokens.json");
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client for OAuth")?;

        let mut manager = Self {
            tokens: HashMap::new(),
            token_file,
            client,
        };

        let _ = manager.load_tokens();

        Ok(manager)
    }

    pub fn load_tokens(&mut self) -> Result<()> {
        if !self.token_file.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&self.token_file)
            .with_context(|| format!("Failed to read token file: {:?}", self.token_file))?;

        self.tokens = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse token file: {:?}", self.token_file))?;

        Ok(())
    }

    pub fn save_tokens(&self) -> Result<()> {
        let content =
            serde_json::to_string_pretty(&self.tokens).context("Failed to serialize tokens")?;

        std::fs::write(&self.token_file, content)
            .with_context(|| format!("Failed to write token file: {:?}", self.token_file))?;

        Ok(())
    }

    pub fn get_token(&self, server_name: &str) -> Option<&OAuthToken> {
        self.tokens.get(server_name)
    }

    pub fn store_token(&mut self, server_name: &str, token: OAuthToken) -> Result<()> {
        self.tokens.insert(server_name.to_string(), token);
        self.save_tokens()
    }

    pub fn is_token_expired(&self, server_name: &str) -> bool {
        if let Some(token) = self.tokens.get(server_name) {
            if let Some(expires_at) = token.expires_at {
                let now = Utc::now().timestamp();
                return now >= expires_at;
            }
            return false;
        }
        true
    }

    pub async fn refresh_token(&mut self, server_name: &str, config: &OAuthConfig) -> Result<()> {
        let token = self
            .tokens
            .get(server_name)
            .with_context(|| format!("No token found for server '{}'", server_name))?;

        let refresh_token = token
            .refresh_token
            .as_ref()
            .with_context(|| format!("No refresh token available for server '{}'", server_name))?;

        let token_url = config
            .token_url
            .as_ref()
            .with_context(|| format!("No token_url configured for server '{}'", server_name))?;

        let mut params = HashMap::new();
        params.insert("grant_type", "refresh_token");
        params.insert("refresh_token", refresh_token.as_str());

        if let Some(client_id) = &config.client_id {
            params.insert("client_id", client_id.as_str());
        }

        if let Some(client_secret) = &config.client_secret {
            params.insert("client_secret", client_secret.as_str());
        }

        let response = self
            .client
            .post(token_url)
            .form(&params)
            .send()
            .await
            .with_context(|| {
                format!("Failed to send token refresh request for '{}'", server_name)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "Token refresh failed for '{}': {} - {}",
                server_name,
                status,
                error_text
            );
        }

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            token_type: String,
            #[serde(default)]
            expires_in: Option<i64>,
            refresh_token: Option<String>,
            scope: Option<String>,
        }

        let token_response: TokenResponse = response
            .json()
            .await
            .with_context(|| format!("Failed to parse token response for '{}'", server_name))?;

        let expires_at = token_response
            .expires_in
            .map(|exp| Utc::now().timestamp() + exp);

        let new_token = OAuthToken {
            access_token: token_response.access_token,
            token_type: token_response.token_type,
            expires_at,
            refresh_token: token_response
                .refresh_token
                .or_else(|| Some(refresh_token.clone())),
            scope: token_response.scope,
        };

        self.store_token(server_name, new_token)?;

        Ok(())
    }

    pub fn get_auth_header(&self, server_name: &str) -> Option<String> {
        self.tokens
            .get(server_name)
            .map(|token| format!("{} {}", token.token_type, token.access_token))
    }
}

impl Default for OAuthManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            let client = Client::new();
            Self {
                tokens: HashMap::new(),
                token_file: PathBuf::from("/dev/null"),
                client,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_oauth_manager() -> (OAuthManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let token_file = temp_dir.path().join("test_tokens.json");
        let client = Client::new();

        let manager = OAuthManager {
            tokens: HashMap::new(),
            token_file,
            client,
        };

        (manager, temp_dir)
    }

    #[test]
    fn test_oauth_token_serialization() {
        let token = OAuthToken {
            access_token: "test_access_token".to_string(),
            token_type: "Bearer".to_string(),
            expires_at: Some(1234567890),
            refresh_token: Some("test_refresh_token".to_string()),
            scope: Some("read write".to_string()),
        };

        let json = serde_json::to_string(&token).unwrap();
        let deserialized: OAuthToken = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.access_token, "test_access_token");
        assert_eq!(deserialized.token_type, "Bearer");
        assert_eq!(deserialized.expires_at, Some(1234567890));
        assert_eq!(
            deserialized.refresh_token,
            Some("test_refresh_token".to_string())
        );
        assert_eq!(deserialized.scope, Some("read write".to_string()));
    }

    #[test]
    fn test_oauth_token_expiry() {
        let (mut manager, _temp_dir) = create_test_oauth_manager();

        let future_timestamp = Utc::now().timestamp() + 3600;
        let future_token = OAuthToken {
            access_token: "future".to_string(),
            token_type: "Bearer".to_string(),
            expires_at: Some(future_timestamp),
            refresh_token: None,
            scope: None,
        };

        manager
            .tokens
            .insert("future_server".to_string(), future_token);
        assert!(!manager.is_token_expired("future_server"));

        let past_timestamp = Utc::now().timestamp() - 3600;
        let past_token = OAuthToken {
            access_token: "past".to_string(),
            token_type: "Bearer".to_string(),
            expires_at: Some(past_timestamp),
            refresh_token: None,
            scope: None,
        };

        manager.tokens.insert("past_server".to_string(), past_token);
        assert!(manager.is_token_expired("past_server"));

        let no_expiry_token = OAuthToken {
            access_token: "no_expiry".to_string(),
            token_type: "Bearer".to_string(),
            expires_at: None,
            refresh_token: None,
            scope: None,
        };

        manager
            .tokens
            .insert("no_expiry_server".to_string(), no_expiry_token);
        assert!(!manager.is_token_expired("no_expiry_server"));

        assert!(manager.is_token_expired("nonexistent_server"));
    }

    #[test]
    fn test_oauth_token_storage() {
        let (mut manager, _temp_dir) = create_test_oauth_manager();

        let token = OAuthToken {
            access_token: "test_token".to_string(),
            token_type: "Bearer".to_string(),
            expires_at: Some(Utc::now().timestamp() + 3600),
            refresh_token: Some("refresh".to_string()),
            scope: None,
        };

        manager.store_token("test_server", token.clone()).unwrap();

        assert!(manager.token_file.exists());

        let loaded_token = manager.get_token("test_server").unwrap();
        assert_eq!(loaded_token.access_token, "test_token");
        assert_eq!(loaded_token.token_type, "Bearer");

        let mut manager2 = OAuthManager {
            tokens: HashMap::new(),
            token_file: manager.token_file.clone(),
            client: Client::new(),
        };

        manager2.load_tokens().unwrap();
        let reloaded_token = manager2.get_token("test_server").unwrap();
        assert_eq!(reloaded_token.access_token, "test_token");
    }

    #[test]
    fn test_oauth_auth_header() {
        let (mut manager, _temp_dir) = create_test_oauth_manager();

        let token = OAuthToken {
            access_token: "my_access_token".to_string(),
            token_type: "Bearer".to_string(),
            expires_at: None,
            refresh_token: None,
            scope: None,
        };

        manager.tokens.insert("test_server".to_string(), token);

        let header = manager.get_auth_header("test_server").unwrap();
        assert_eq!(header, "Bearer my_access_token");

        assert!(manager.get_auth_header("nonexistent").is_none());
    }

    #[test]
    fn test_oauth_config_serialization() {
        let config = OAuthConfig {
            client_id: Some("client123".to_string()),
            client_secret: Some("secret456".to_string()),
            auth_url: Some("https://auth.example.com/oauth/authorize".to_string()),
            token_url: Some("https://auth.example.com/oauth/token".to_string()),
            scopes: vec!["read".to_string(), "write".to_string()],
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: OAuthConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.client_id, Some("client123".to_string()));
        assert_eq!(deserialized.client_secret, Some("secret456".to_string()));
        assert_eq!(deserialized.scopes, vec!["read", "write"]);
    }

    #[test]
    fn test_oauth_manager_new() {
        let manager = OAuthManager::new();
        assert!(manager.is_ok());

        let manager = manager.unwrap();
        assert!(manager.tokens.is_empty());
        assert!(manager.token_file.to_string_lossy().contains("openrust"));
        assert!(manager
            .token_file
            .to_string_lossy()
            .contains("mcp_tokens.json"));
    }

    #[test]
    fn test_oauth_token_without_optional_fields() {
        let token = OAuthToken {
            access_token: "minimal".to_string(),
            token_type: "Bearer".to_string(),
            expires_at: None,
            refresh_token: None,
            scope: None,
        };

        let json = serde_json::to_string(&token).unwrap();
        assert!(!json.contains("expires_at"));
        assert!(!json.contains("refresh_token"));
        assert!(!json.contains("scope"));

        let deserialized: OAuthToken = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.access_token, "minimal");
        assert_eq!(deserialized.token_type, "Bearer");
        assert!(deserialized.expires_at.is_none());
        assert!(deserialized.refresh_token.is_none());
        assert!(deserialized.scope.is_none());
    }
}
