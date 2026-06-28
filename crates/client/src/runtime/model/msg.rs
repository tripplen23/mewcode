use uuid::Uuid;

use crossterm::event::{KeyEvent, MouseEvent};
use mewcode_protocol::canvas::{Graph, Layout};

use crate::net::{ModelEntry, Session, SessionSummary};

/// Messages that drive the [`super::App`] through `update`.
#[derive(Debug)]
pub enum Msg {
    /// A key was pressed.
    Key(KeyEvent),
    /// A mouse event arrived.
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
    /// The project's canvas finished loading (both graph and
    /// layout fetched in parallel). A failure short-circuits —
    /// `Err` means at least one of the two HTTP calls failed; the
    /// Canvas screen surfaces a toast and stays in its previous
    /// state.
    CanvasLoaded(Result<CanvasData, String>),
}

/// Result of a successful canvas load: the graph + the layout
/// (positions + theme) as fetched from the server. Auto-layout is
/// *not* applied at this layer — the view layer resolves missing
/// positions with the engine's `auto_layout` after both fields
/// land, so the load stays a pure wire-format deserialization.
#[derive(Debug, Clone)]
pub struct CanvasData {
    /// Semantic graph (source of truth).
    pub graph: Graph,
    /// Presentation overlay (positions + theme).
    pub layout: Layout,
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

impl std::fmt::Display for CreateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CreateError::EmptyTitle(s) => write!(f, "{s}"),
            CreateError::Other(s) => write!(f, "{s}"),
        }
    }
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
