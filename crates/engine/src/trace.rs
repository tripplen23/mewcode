//! Tracing setup.

use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Default `tracing` filter when `RUST_LOG` is unset.
pub const DEFAULT_LOG: &str = "info";

// ---------------------------------------------------------------------------
// OpenTelemetry / Langfuse semantic convention identifiers
//
// These are referenced by the harness and must stay in sync with
// <https://opentelemetry.io/docs/specs/semconv/gen-ai/> and
// <https://langfuse.com/docs/opentelemetry/get-started/>.
// ---------------------------------------------------------------------------

/// Span name for a single agent turn (also the Langfuse trace name).
pub const TRACE_NAME_CHAT_TURN: &str = "chat-turn";

/// Gen AI operation name (`gen_ai.operation.name`). Wraps an agent invocation.
pub const GEN_AI_OP_INVOKE_AGENT: &str = "invoke_agent";

/// Agent identity (`gen_ai.agent.name`).
pub const GEN_AI_AGENT_MEWCODE: &str = "mewcode";

/// Provider identity (`gen_ai.provider.name`).
pub const GEN_AI_PROVIDER_OPENCODE_GO: &str = "opencode-go";

/// Langfuse observation type for LLM generations.
pub const LANGFUSE_OBSERVATION_GENERATION: &str = "generation";

/// Role strings used in observation [`serde_json::Value`] payloads.
pub const GEN_AI_ROLE_USER: &str = "user";
pub const GEN_AI_ROLE_ASSISTANT: &str = "assistant";

// ---------------------------------------------------------------------------
// Span-attribute field names passed to [`tracing::Span::record`].
//
// Each corresponds to a field declared in the `chat-turn`
// [`tracing::info_span!`] macro inside [`crate::harness::Harness`]; mismatches
// are silently dropped by tracing.
// ---------------------------------------------------------------------------

/// `gen_ai.system_instructions` — system prompt.
pub const FIELD_GEN_AI_SYSTEM_INSTRUCTIONS: &str = "gen_ai.system_instructions";
/// `gen_ai.prompt` — user prompt.
pub const FIELD_GEN_AI_PROMPT: &str = "gen_ai.prompt";
/// `gen_ai.completion` — assistant reply.
pub const FIELD_GEN_AI_COMPLETION: &str = "gen_ai.completion";
/// `langfuse.trace.input` — trace-level input text.
pub const FIELD_LANGFUSE_TRACE_INPUT: &str = "langfuse.trace.input";
/// `langfuse.trace.output` — trace-level output text.
pub const FIELD_LANGFUSE_TRACE_OUTPUT: &str = "langfuse.trace.output";
/// `langfuse.observation.input` — generation-observation input
/// (JSON-encoded `{"role": "user", "content": "..."}`).
pub const FIELD_LANGFUSE_OBSERVATION_INPUT: &str = "langfuse.observation.input";
/// `langfuse.observation.output` — generation-observation output
/// (JSON-encoded `{"role": "assistant", "content": "..."}`).
pub const FIELD_LANGFUSE_OBSERVATION_OUTPUT: &str = "langfuse.observation.output";
/// `input.value` — duplicate of [`FIELD_LANGFUSE_TRACE_INPUT`] for
/// OpenInference compatibility.
pub const FIELD_INPUT_VALUE: &str = "input.value";
/// `output.value` — duplicate of [`FIELD_LANGFUSE_TRACE_OUTPUT`] for
/// OpenInference compatibility.
pub const FIELD_OUTPUT_VALUE: &str = "output.value";

/// Initialise a global `tracing` subscriber for the engine.
///
/// Honours `RUST_LOG`. Safe to call multiple times — only the first call
/// has any effect.
pub fn init() {
    let _ = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG)))
        .with(fmt::layer().with_target(true).with_level(true))
        .try_init();
}

/// Initialise a JSON-formatted file appender at the given path, suitable
/// for the TUI's trace pane to tail.
pub fn init_json_file(path: &std::path::Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let (file, guard) = tracing_appender::non_blocking(file);
    let _ = guard; // leak on purpose: tracing is process-global
    let _ = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG)))
        .with(fmt::layer().json().with_writer(file))
        .try_init();
    Ok(())
}
