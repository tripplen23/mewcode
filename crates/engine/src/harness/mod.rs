//! Long-running agent harness. Owns the conversation state, drives
//! the tool-calling loop, and streams [`mewcode_protocol::StreamEvent`]s
//! back through an mpsc channel until the model stops emitting tool
//! calls or the user cancels.

mod completion;

pub use self::completion::{last_user_text, reply_text};

use std::sync::Arc;

use mewcode_protocol::{Message, Mode, ModelId, StreamEvent};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::agent::build_system_prompt;
use crate::config::EngineConfig;
use crate::error::EngineError;
use crate::langfuse::{LangfuseTracer, TurnReport};
use crate::provider::Provider;
use crate::skills::SkillRegistry;
use crate::tools::ToolRegistry;

/// The agent harness.
#[derive(Clone)]
pub struct Harness {
    model: ModelId,
    mode: Mode,
    cancel: CancellationToken,
    skills: Arc<SkillRegistry>,
    tools: Arc<ToolRegistry>,
    tracer: Option<Arc<LangfuseTracer>>,
    session_id: Option<Uuid>,
}

impl std::fmt::Debug for Harness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Harness")
            .field("model", &self.model)
            .field("mode", &self.mode)
            .field("tools", &self.tools.names())
            .field("skill_count", &self.skills.len())
            .finish()
    }
}

impl Harness {
    /// Build a new harness. `skills` is the catalog source for the
    /// system prompt; `tools` supplies the descriptors the model can call.
    pub fn new(
        model: ModelId,
        mode: Mode,
        skills: Arc<SkillRegistry>,
        tools: Arc<ToolRegistry>,
    ) -> Self {
        Self {
            model,
            mode,
            cancel: CancellationToken::new(),
            skills,
            tools,
            tracer: None,
            session_id: None,
        }
    }

    /// Attach an optional Langfuse tracer. Passing `None` (or never calling
    /// this) leaves tracing disabled.
    pub fn with_tracer(mut self, tracer: Option<Arc<LangfuseTracer>>) -> Self {
        self.tracer = tracer;
        self
    }

    /// Record the chat session id so reported turns are grouped by session in Langfuse.
    pub fn with_session(mut self, session_id: Uuid) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Cancel the in-flight stream, if any.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    /// The system prompt for the model's first turn.
    pub fn system_prompt(&self) -> String {
        build_system_prompt(self.mode, &self.skills, &self.tools)
    }

    /// Number of skills currently available.
    pub fn skill_count(&self) -> usize {
        self.skills.len()
    }

    /// Tool names currently registered.
    pub fn tool_names(&self) -> Vec<&'static str> {
        self.tools.names()
    }

    /// Run one completion against OpenCode Go and stream the reply as
    /// `Start` → `TextDelta`* → `Finish`. Returns `Err` on any failure and
    /// emits nothing on that path — the caller owns the `Error` event.
    pub async fn run_turn(
        &self,
        messages: &[Message],
        tx: mpsc::Sender<StreamEvent>,
    ) -> Result<(), EngineError> {
        let start = chrono::Utc::now();
        let result = self.run_turn_inner(messages, &tx).await;
        self.trace_turn(messages, &result, start);
        result.map(|_| ())
    }

    /// The turn proper: resolve config, select the user message, run one
    /// completion, and emit the success-path events. Returns the assistant
    /// reply on success so the caller can both report it and discard it. The
    /// SSE emission is unchanged — nothing reaches the channel on failure, so
    /// the server route stays the single owner of the `Error` event.
    async fn run_turn_inner(
        &self,
        messages: &[Message],
        tx: &mpsc::Sender<StreamEvent>,
    ) -> Result<String, EngineError> {
        // Resolve the credential first: a missing key must fail before any
        // provider is constructed or any request is built.
        let cfg = EngineConfig::from_env()?;

        // The turn always answers the most recent user message. With no
        // user message there is nothing to send, so fail without a provider.
        let user_text = last_user_text(messages)
            .ok_or_else(|| EngineError::Other("no user message in chat history".to_string()))?;

        let provider = Provider::for_model(self.model, &cfg.api_key, &cfg.base_url)?;
        let system_prompt = build_system_prompt(self.mode, &self.skills, &self.tools);

        // Exactly one completion, no tool dispatch, no follow-up turn.
        let reply = self.complete(provider, system_prompt, user_text).await?;

        // Only on success do we emit anything, so a failed completion leaves the
        // channel untouched for the caller's single `Error` event.
        self.emit_reply(&reply, tx).await?;
        Ok(reply)
    }

    /// Fire-and-forget the turn's outcome to Langfuse when a tracer is
    /// configured. Reporting runs on its own task and never affects the turn's
    /// result or the SSE stream.
    fn trace_turn(
        &self,
        messages: &[Message],
        result: &Result<String, EngineError>,
        start: chrono::DateTime<chrono::Utc>,
    ) {
        let Some(tracer) = &self.tracer else {
            return;
        };
        let report = TurnReport {
            session_id: self.session_id.map(|id| id.to_string()),
            model: self.model.provider_id().to_string(),
            mode: format!("{:?}", self.mode),
            input: last_user_text(messages).unwrap_or_default(),
            outcome: match result {
                Ok(reply) => Ok(reply.clone()),
                Err(e) => Err(e.to_string()),
            },
            start,
            end: chrono::Utc::now(),
        };
        let tracer = tracer.clone();
        tokio::spawn(async move { tracer.report_turn(report).await });
    }

    /// Run exactly one completion through the routed provider and fold its text
    /// segments into one reply string. Kept off the emission path so `run_turn`
    /// emits nothing until a reply exists.
    async fn complete(
        &self,
        provider: Provider,
        system_prompt: String,
        user_text: String,
    ) -> Result<String, EngineError> {
        let choice = provider
            .complete(
                self.model.provider_id(),
                system_prompt,
                user_text,
                Self::MAX_TOKENS,
            )
            .await?;
        Ok(reply_text(&choice))
    }

    /// Output token cap for a single turn.
    const MAX_TOKENS: u64 = 4096;

    /// Emit the success-path event sequence for one turn: exactly one `Start`
    /// carrying this turn's mode and model, then a single `TextDelta` (omitted
    /// when `reply` is empty), then exactly one `Finish`, with zero tool events.
    pub async fn emit_reply(
        &self,
        reply: &str,
        tx: &mpsc::Sender<StreamEvent>,
    ) -> Result<(), EngineError> {
        let started = std::time::Instant::now();
        let message_id = uuid::Uuid::new_v4();

        tx.send(StreamEvent::Start {
            message_id,
            mode: self.mode,
            model: self.model,
        })
        .await
        .map_err(|e| EngineError::Other(e.to_string()))?;

        if !reply.is_empty() {
            tx.send(StreamEvent::TextDelta {
                delta: reply.to_string(),
            })
            .await
            .map_err(|e| EngineError::Other(e.to_string()))?;
        }

        tx.send(StreamEvent::Finish {
            duration_ms: started.elapsed().as_millis() as u64,
            input_tokens: None,
            output_tokens: None,
        })
        .await
        .map_err(|e| EngineError::Other(e.to_string()))?;

        Ok(())
    }
}
