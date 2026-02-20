pub mod anthropic;
pub mod openai;
pub mod types;

use async_trait::async_trait;
use tokio::sync::mpsc;

use types::{AiError, CompletionRequest, CompletionResponse};

#[async_trait]
pub trait Provider: Send + Sync {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, AiError>;

    async fn complete_stream(
        &self,
        request: CompletionRequest,
        tx: mpsc::Sender<Result<String, AiError>>,
    ) -> Result<(), AiError>;

    fn name(&self) -> &str;
}

pub fn create_provider(
    provider_name: &str,
    api_key: String,
    base_url: Option<String>,
) -> Box<dyn Provider> {
    match provider_name {
        "anthropic" => Box::new(anthropic::AnthropicProvider::new(api_key)),
        "openai" => Box::new(openai::OpenAIProvider::new(api_key, base_url)),
        _ => Box::new(anthropic::AnthropicProvider::new(api_key)),
    }
}
