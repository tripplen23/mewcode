//! OpenCode Go provider routing. Hides which endpoint (`/v1/messages`
//! vs `/v1/chat/completions`) a given [`ModelId`] needs so the rest of
//! the engine can ask for a provider by model alone.
//!
//! Thin wrappers over [rig-core](https://docs.rs/rig-core/latest/rig_core/)'
//! [Anthropic](https://docs.rs/rig-core/latest/rig_core/providers/anthropic/index.html)
//! and [OpenAI](https://docs.rs/rig-core/latest/rig_core/providers/openai/index.html)
//! provider clients. The rest of the engine drives Rig's
//! [`Agent`](https://docs.rs/rig-core/latest/rig_core/agent/struct.Agent.html)
//! abstraction; the provider arms here are only a routing shim that selects the
//! right Rig client for the model kind.

use futures::StreamExt;
use mewcode_protocol::{ModelId, ModelKind, StreamEvent};
use rig_core::agent::MultiTurnStreamItem;
use rig_core::client::CompletionClient;
use rig_core::completion::Prompt;
use rig_core::streaming::StreamedAssistantContent;
use rig_core::streaming::StreamingPrompt;
use tokio::sync::mpsc;

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

    /// Build and invoke a Rig agent for one user prompt, with conversation
    /// history so follow-up questions have context.
    ///
    /// The two provider arms stay explicit because they use different OpenCode
    /// Go wire protocols, but both go through Rig's `Agent` abstraction. That
    /// keeps the harness ready for the next phase: tools, skills, and streaming
    /// can attach to the agent builder/request instead of a low-level completion
    /// request.
    #[allow(clippy::too_many_arguments)]
    pub async fn invoke_agent(
        &self,
        model_id: &str,
        system_prompt: String,
        history: Vec<rig_core::completion::Message>,
        user_text: String,
        max_tokens: u64,
        max_turns: usize,
    ) -> Result<String, EngineError> {
        let reply = match self {
            Provider::Anthropic(p) => {
                let agent = p
                    .client()
                    .agent(model_id)
                    .name("mewcode")
                    .preamble(&system_prompt)
                    .max_tokens(max_tokens)
                    .default_max_turns(max_turns)
                    .build();

                agent
                    .prompt(user_text)
                    .with_history(history)
                    .await
                    .map_err(|e| EngineError::Other(e.to_string()))?
            }
            Provider::OpenAi(p) => {
                let agent = p
                    .client()
                    .agent(model_id)
                    .name("mewcode")
                    .preamble(&system_prompt)
                    .max_tokens(max_tokens)
                    .default_max_turns(max_turns)
                    .build();

                agent
                    .prompt(user_text)
                    .with_history(history)
                    .await
                    .map_err(|e| EngineError::Other(e.to_string()))?
            }
        };
        Ok(reply)
    }

    /// Like [`invoke_agent`] but streams `TextDelta` events as text chunks
    /// arrive, emitting them through `tx`. Tools are passed to the Rig
    /// agent builder so the model can call them during multi-turn loops.
    pub async fn invoke_agent_streaming(
        &self,
        req: AgentRequest<'_>,
        tx: &mpsc::Sender<StreamEvent>,
    ) -> Result<String, EngineError> {
        match self {
            Provider::Anthropic(p) => {
                let agent = p
                    .client()
                    .agent(req.model_id)
                    .name("mewcode")
                    .preamble(&req.system_prompt)
                    .max_tokens(req.max_tokens)
                    .default_max_turns(req.max_turns)
                    .tools(req.tools)
                    .build();
                stream_agent_completion(agent, req.user_text, req.history, tx).await
            }
            Provider::OpenAi(p) => {
                let agent = p
                    .client()
                    .agent(req.model_id)
                    .name("mewcode")
                    .preamble(&req.system_prompt)
                    .max_tokens(req.max_tokens)
                    .default_max_turns(req.max_turns)
                    .tools(req.tools)
                    .build();
                stream_agent_completion(agent, req.user_text, req.history, tx).await
            }
        }
    }
}

/// Inputs to [`Provider::invoke_agent_streaming`].  Bundled into a struct
/// to keep argument counts below clippy's threshold and to make call-sites
/// self-documenting.
pub struct AgentRequest<'a> {
    /// Model identifier (e.g. `"claude-sonnet-4"`).
    pub model_id: &'a str,
    /// System prompt prepended to the conversation.
    pub system_prompt: String,
    /// Prior conversation history (oldest → newest).
    pub history: Vec<rig_core::completion::Message>,
    /// The current user message.
    pub user_text: String,
    /// Cap on completion tokens per turn.
    pub max_tokens: u64,
    /// Cap on agent-internal turns before stopping.
    pub max_turns: usize,
    /// Tools the agent can call during the turn (Rig `ToolDyn` wrappers).
    pub tools: Vec<Box<dyn rig_core::tool::ToolDyn>>,
}

/// Generic streaming helper for any Rig `Agent` with the default hook (`()`).
///
/// Emits `TextDelta` for text chunks, `ToolInputAvailable` when the model
/// requests a tool call, and `ToolOutputAvailable` when the tool result is
/// fed back. The multi-turn loop is handled by Rig internally.
async fn stream_agent_completion<M: rig_core::completion::CompletionModel + 'static>(
    agent: rig_core::agent::Agent<M, ()>,
    user_text: String,
    history: Vec<rig_core::completion::Message>,
    tx: &mpsc::Sender<StreamEvent>,
) -> Result<String, EngineError> {
    let mut stream = agent.stream_prompt(user_text).with_history(history).await;

    let mut full_reply = String::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(t))) => {
                let delta = t.text;
                let _ = tx
                    .send(StreamEvent::TextDelta {
                        delta: delta.clone(),
                    })
                    .await;
                full_reply.push_str(&delta);
            }
            Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::ToolCall {
                tool_call,
                ..
            })) => {
                let _ = tx
                    .send(StreamEvent::ToolInputAvailable {
                        tool_call_id: tool_call.id.clone(),
                        tool_name: tool_call.function.name.clone(),
                        input: tool_call.function.arguments.clone(),
                    })
                    .await;
            }
            Ok(MultiTurnStreamItem::StreamUserItem(user_content)) => {
                // StreamedUserContent has a single variant (ToolResult), so
                // we destructure directly.
                let rig_core::streaming::StreamedUserContent::ToolResult { tool_result, .. } =
                    user_content;
                let output = tool_result
                    .content
                    .iter()
                    .find_map(|c| match c {
                        rig_core::completion::message::ToolResultContent::Text(t) => {
                            Some(t.text.clone())
                        }
                        _ => None,
                    })
                    .unwrap_or_default();
                let parsed = serde_json::from_str::<serde_json::Value>(&output)
                    .unwrap_or(serde_json::Value::String(output));
                let _ = tx
                    .send(StreamEvent::ToolOutputAvailable {
                        tool_call_id: tool_result.id,
                        output: parsed,
                    })
                    .await;
            }
            Ok(MultiTurnStreamItem::CompletionCall(call)) => {
                // Record usage on the current tracing span if available.
                if let Some(usage) = &call.usage {
                    tracing::debug!(
                        input_tokens = usage.input_tokens,
                        output_tokens = usage.output_tokens,
                        "completion call usage"
                    );
                }
            }
            Ok(MultiTurnStreamItem::FinalResponse(response)) => {
                if full_reply.is_empty() {
                    let text = response.response().to_string();
                    if !text.is_empty() {
                        let _ = tx
                            .send(StreamEvent::TextDelta {
                                delta: text.clone(),
                            })
                            .await;
                        full_reply = text;
                    }
                }
            }
            Err(e) => return Err(EngineError::Other(e.to_string())),
            _ => {}
        }
    }
    Ok(full_reply)
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

/// OpenAI-compatible provider. Wraps rig-core's chat-completions client.
#[derive(Clone)]
pub struct OpenAiProvider {
    client: rig_core::providers::openai::CompletionsClient,
}

impl OpenAiProvider {
    /// Build a new provider.
    pub fn new(api_key: &str, base_url: &str) -> Self {
        let client = rig_core::providers::openai::CompletionsClient::builder()
            .api_key(api_key)
            .base_url(base_url)
            .build()
            .expect("openai client build is infallible");
        Self { client }
    }

    /// Borrow the underlying rig client.
    pub fn client(&self) -> &rig_core::providers::openai::CompletionsClient {
        &self.client
    }
}
