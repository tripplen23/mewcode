use std::time::Instant;

use tui_textarea::TextArea;
use uuid::Uuid;

use crate::net::Session;

/// An overlay panel layered over the session view.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    /// No overlay.
    #[default]
    None,
    /// The tools list overlay.
    Tools,
    /// The skills list overlay.
    Skills,
}

/// State backing [`super::Screen::Session`].
#[derive(Debug)]
pub struct SessionState {
    /// The hydrated session, including history. Cannot be omitted.
    pub session: Session,
    /// The message composer.
    pub input: TextArea<'static>,
    /// Vertical scroll offset of the transcript, in wrapped lines from the top.
    pub scroll: u16,
    /// When `true`, the transcript stays pinned to its latest line.
    pub follow: bool,
    /// Largest valid `scroll` for the last rendered frame (content lines minus
    /// viewport height). Written by the view, read by the key handler so it can
    /// clamp scrolling and know when the bottom has been reached.
    pub max_scroll: u16,
    /// Transcript viewport height from the last rendered frame, used as the
    /// PageUp/PageDown step.
    pub viewport: u16,
    /// `Some` while an assistant turn is in flight.
    pub streaming: Option<StreamingState>,
    /// Which overlay (if any) is showing.
    pub overlay: Overlay,
}

impl SessionState {
    /// Open a session view for an already-hydrated [`Session`].
    pub fn new(session: Session) -> Self {
        Self {
            session,
            input: TextArea::default(),
            scroll: 0,
            follow: true,
            max_scroll: 0,
            viewport: 0,
            streaming: None,
            overlay: Overlay::None,
        }
    }
}

/// A lightweight view of a tool call accumulated during streaming.
#[derive(Debug, Clone)]
pub struct ToolCallView {
    /// Stable id of the call.
    pub id: String,
    /// Tool name.
    pub name: String,
    /// JSON arguments.
    pub input: serde_json::Value,
    /// JSON output, once the call finishes.
    pub output: Option<serde_json::Value>,
}

/// State of an in-flight assistant turn.
#[derive(Debug)]
pub struct StreamingState {
    /// Id of the assistant message being produced.
    pub assistant_id: Uuid,
    /// Accumulated assistant text so far.
    pub buffer: String,
    /// Tool calls seen during this turn.
    pub tool_calls: Vec<ToolCallView>,
    /// When the turn started (for elapsed-time display / animations).
    pub started_at: Instant,
}

impl StreamingState {
    /// Begin tracking a new assistant turn.
    pub fn new(assistant_id: Uuid) -> Self {
        Self {
            assistant_id,
            buffer: String::new(),
            tool_calls: Vec::new(),
            started_at: Instant::now(),
        }
    }
}
