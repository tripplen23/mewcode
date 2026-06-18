//! OpenCode Go provider routing. Hides which endpoint (`/v1/messages`
//! vs `/v1/chat/completions`) a given [`ModelId`] needs so the rest of
//! the engine can ask for a provider by model alone.
//!
//! Thin wrappers over [rig-core](https://docs.rs/rig-core/latest/rig_core/)'
//! [Anthropic](https://docs.rs/rig-core/latest/rig_core/providers/anthropic/index.html)
//! and [OpenAI](https://docs.rs/rig-core/latest/rig_core/providers/openai/index.html)
//! provider clients. The rig
//! [`CompletionModel`](https://docs.rs/rig-core/latest/rig_core/completion/request/trait.CompletionModel.html)
//! trait is what the rest of the engine drives; the provider arms here
//! are a routing shim that selects the right rig client for the model kind.

use mewcode_protocol::{ModelId, ModelKind};
use rig_core::OneOrMany;
use rig_core::client::CompletionClient;
use rig_core::completion::CompletionModel;
use rig_core::completion::message::AssistantContent;

use crate::error::EngineError;

/// A provider client capable of issuing chat-completion requests to OpenCode Go.
#[derive(Clone)]
pub enum Provider {
    /// Anthropic-compatible provider, hits `/v1/messages`.
    Anthropic(AnthropicProvider),
    /// OpenAI-compatible provider, hits `/v1/chat/completions`.
    OpenAi(OpenAiProvider),
}

impl Provider {
    /// Build a provider for the given model.
    pub fn for_model(model: ModelId, api_key: &str, base_url: &str) -> Result<Self, EngineError> {
        let provider = match model.kind() {
            ModelKind::AnthropicMessages => {
                Provider::Anthropic(AnthropicProvider::new(api_key, base_url))
            }
            ModelKind::OpenAiChatCompletions => {
                Provider::OpenAi(OpenAiProvider::new(api_key, base_url))
            }
        };
        Ok(provider)
    }

    /// Run one completion and return its choice. Both provider arms are
    /// mechanically identical (same request shape, same response type) so they
    /// sit side-by-side here, where a reviewer can see "these are identical"
    /// and a future change is forced to update both.
    pub async fn complete(
        &self,
        model_id: &str,
        system_prompt: String,
        user_text: String,
        max_tokens: u64,
    ) -> Result<OneOrMany<AssistantContent>, EngineError> {
        let choice = match self {
            Provider::Anthropic(p) => {
                p.client()
                    .completion_model(model_id)
                    .completion_request(user_text)
                    .preamble(system_prompt)
                    .max_tokens(max_tokens)
                    .send()
                    .await
                    .map_err(|e| EngineError::Other(e.to_string()))?
                    .choice
            }
            Provider::OpenAi(p) => {
                p.client()
                    .completion_model(model_id)
                    .completion_request(user_text)
                    .preamble(system_prompt)
                    .max_tokens(max_tokens)
                    .send()
                    .await
                    .map_err(|e| EngineError::Other(e.to_string()))?
                    .choice
            }
        };
        Ok(choice)
    }
}

/// Anthropic-compatible provider. Wraps rig-core's
/// [`anthropic::Client`](https://docs.rs/rig-core/latest/rig_core/providers/anthropic/client/index.html#typealias.Client).
#[derive(Clone)]
pub struct AnthropicProvider {
    client: rig_core::providers::anthropic::Client,
}

impl AnthropicProvider {
    /// Build a new provider.
    pub fn new(api_key: &str, base_url: &str) -> Self {
        let client = rig_core::providers::anthropic::Client::builder()
            .api_key(api_key)
            .base_url(base_url)
            .build()
            .expect("anthropic client build is infallible");
        Self { client }
    }

    /// Borrow the underlying rig client.
    pub fn client(&self) -> &rig_core::providers::anthropic::Client {
        &self.client
    }
}

/// OpenAI-compatible provider. Wraps rig-core's
/// [`openai::Client`](https://docs.rs/rig-core/latest/rig_core/providers/openai/client/index.html#typealias.Client).
#[derive(Clone)]
pub struct OpenAiProvider {
    client: rig_core::providers::openai::Client,
}

impl OpenAiProvider {
    /// Build a new provider.
    pub fn new(api_key: &str, base_url: &str) -> Self {
        let client = rig_core::providers::openai::Client::builder()
            .api_key(api_key)
            .base_url(base_url)
            .build()
            .expect("openai client build is infallible");
        Self { client }
    }

    /// Borrow the underlying rig client.
    pub fn client(&self) -> &rig_core::providers::openai::Client {
        &self.client
    }
}
