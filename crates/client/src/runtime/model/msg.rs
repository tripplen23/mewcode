use uuid::Uuid;

use crossterm::event::{KeyEvent, MouseEvent};

use crate::net::{ModelEntry, Session, SessionSummary};

/// Messages that drive the [`super::App`] through `update`.
#[derive(Debug)]
pub enum Msg {
    /// A key was pressed.
    Key(KeyEvent),
    /// A mouse event arrived. Currently ignored by every screen
    /// (no behaviour change); the variant exists so T5 (canvas
    /// navigation) can attach handlers in a follow-up PR.
    Mouse(MouseEvent),
    /// A periodic tick (for animations / elapsed time).
    Tick,
    /// The session list finished loading.
    SessionsLoaded(Result<Vec<SessionSummary>, String>),
    /// The model registry finished loading.
    ModelsLoaded(Result<Vec<ModelEntry>, String>),
    /// A new session finished being created.
    SessionCreated(Result<Session, CreateError>),
    /// A session finished being opened/hydrated.
    SessionOpened(Result<Session, String>),
    /// A streaming event arrived.
    Stream(StreamMsg),
}

/// Why a `POST /sessions` failed.
///
/// Distinguishes the empty-title client error (keep focus + hint) from every
/// other failure (persistent error, retain input) so `update` can branch
/// without re-deriving HTTP semantics.
#[derive(Debug)]
pub enum CreateError {
    /// The server rejected the request because the title was empty.
    EmptyTitle(String),
    /// Any other failure (transport, decode, non-4xx status).
    Other(String),
}

/// Streaming sub-messages, decoded from server SSE events.
#[derive(Debug)]
pub enum StreamMsg {
    /// Stream started; carries the assistant message id.
    Started(Uuid),
    /// A chunk of assistant text.
    Delta(String),
    /// The model is calling a tool.
    ToolInput {
        /// Stable id of the call.
        id: String,
        /// Tool name.
        name: String,
        /// JSON arguments.
        input: serde_json::Value,
    },
    /// A tool call produced output.
    ToolOutput {
        /// Id of the call this result is for.
        id: String,
        /// JSON output.
        output: serde_json::Value,
    },
    /// Stream finished successfully.
    Finished {
        /// Wall-clock duration in milliseconds.
        duration_ms: u64,
    },
    /// Stream failed.
    Failed(String),
}
