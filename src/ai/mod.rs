pub mod anthropic;
pub mod azure;
pub mod bedrock;
pub mod google;
pub mod openai;
pub mod types;

use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::mpsc;

use types::{AiError, CompletionRequest, CompletionResponse, StreamEvent};

#[async_trait]
pub trait Provider: Send + Sync {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, AiError>;

    async fn complete_stream(
        &self,
        request: CompletionRequest,
        tx: mpsc::Sender<Result<StreamEvent, AiError>>,
    ) -> Result<(), AiError>;

    fn name(&self) -> &str;
}

/// Additional options for providers that need extra config beyond api_key/base_url
#[derive(Debug, Clone, Default)]
pub struct ProviderOptions {
    pub region: Option<String>,
    pub secret_key: Option<String>,
    pub session_token: Option<String>,
    pub model_id: Option<String>,
    pub endpoint: Option<String>,
    pub deployment: Option<String>,
    pub api_version: Option<String>,
}

pub fn create_provider(
    provider_name: &str,
    api_key: String,
    base_url: Option<String>,
    extra_headers: Option<HashMap<String, String>>,
    provider_options: Option<ProviderOptions>,
) -> Result<Box<dyn Provider>, String> {
    match provider_name {
        "anthropic" => Ok(Box::new(anthropic::AnthropicProvider::new(api_key))),
        "openai" => Ok(Box::new(openai::OpenAIProvider::new(
            api_key,
            base_url,
            extra_headers,
        ))),
        "google" | "gemini" => Ok(Box::new(google::GeminiProvider::new(api_key))),
        "bedrock" | "aws-bedrock" => {
            let opts = provider_options.ok_or("Bedrock requires provider_options")?;
            Ok(Box::new(bedrock::BedrockProvider::new(
                opts.region.unwrap_or_else(|| "us-east-1".to_string()),
                api_key,
                opts.secret_key.unwrap_or_default(),
                opts.session_token,
                opts.model_id.unwrap_or_default(),
            )))
        }
        "azure" | "azure-openai" => {
            let opts = provider_options.ok_or("Azure requires provider_options")?;
            Ok(Box::new(azure::AzureOpenAIProvider::new(
                api_key,
                opts.endpoint.unwrap_or_default(),
                opts.deployment.unwrap_or_default(),
                opts.api_version,
            )))
        }
        "openrouter" => Ok(Box::new(openai::OpenAIProvider::openrouter(api_key))),
        "ollama" => Ok(Box::new(openai::OpenAIProvider::ollama(base_url))),
        "generic" | "openai-compatible" => {
            let url = base_url.ok_or("Generic provider requires base_url")?;
            Ok(Box::new(openai::OpenAIProvider::generic(
                api_key,
                url,
                extra_headers,
            )))
        }
        _ => Err(format!("Unknown provider: {}", provider_name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_provider_anthropic() {
        let provider =
            create_provider("anthropic", "test_key".to_string(), None, None, None).unwrap();
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn test_create_provider_openai() {
        let provider = create_provider("openai", "test_key".to_string(), None, None, None).unwrap();
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_create_provider_google() {
        let provider = create_provider("google", "test_key".to_string(), None, None, None).unwrap();
        assert_eq!(provider.name(), "google");
    }

    #[test]
    fn test_create_provider_gemini_alias() {
        let provider = create_provider("gemini", "test_key".to_string(), None, None, None).unwrap();
        assert_eq!(provider.name(), "google");
    }

    #[test]
    fn test_create_provider_openrouter() {
        let provider =
            create_provider("openrouter", "test_key".to_string(), None, None, None).unwrap();
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_create_provider_ollama() {
        let provider = create_provider("ollama", String::new(), None, None, None).unwrap();
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_create_provider_unknown() {
        let result = create_provider("unknown", "test_key".to_string(), None, None, None);
        assert!(result.is_err());
        match result {
            Err(e) => assert_eq!(e, "Unknown provider: unknown"),
            Ok(_) => panic!("Expected error for unknown provider"),
        }
    }

    #[test]
    fn test_create_provider_bedrock() {
        let opts = ProviderOptions {
            region: Some("us-west-2".to_string()),
            secret_key: Some("secret".to_string()),
            session_token: None,
            model_id: Some("anthropic.claude-v2".to_string()),
            ..Default::default()
        };
        let provider = create_provider(
            "bedrock",
            "access_key".to_string(),
            None,
            None,
            Some(opts),
        )
        .unwrap();
        assert_eq!(provider.name(), "bedrock");
    }

    #[test]
    fn test_create_provider_azure() {
        let opts = ProviderOptions {
            endpoint: Some("https://my-resource.openai.azure.com".to_string()),
            deployment: Some("gpt-4".to_string()),
            api_version: Some("2023-05-15".to_string()),
            ..Default::default()
        };
        let provider =
            create_provider("azure", "test_key".to_string(), None, None, Some(opts)).unwrap();
        assert_eq!(provider.name(), "azure");
    }
}
