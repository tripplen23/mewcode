//! The application's state types, grouped by where they live.
//!
//! `App` and `Screen` are the top-level state machine; `Toast` is a
//! cross-screen UI primitive. The remaining structs are split per screen
//! (matching the layout in [`super::super::view`] and [`super::super::update`])
//! so the file you open matches the file you'd change.

use std::time::Instant;

use mewcode_protocol::canvas::NodeId;
use mewcode_protocol::canvas::{Graph, Layout};

mod home;
mod new_session;
mod session;

pub use home::HomeState;
pub use new_session::{ModelPicker, NewSessionField, NewSessionState};
pub use session::{Overlay, SessionState, StreamingState, ToolCallView};

/// The whole application state.
///
/// The current view is held solely as a single [`Screen`] value; there is no
/// screen-specific data outside its variant.
#[derive(Debug)]
pub struct App {
    /// The screen currently being shown, owning its own state.
    pub screen: Screen,
    /// Transient status message, if any.
    pub toast: Option<Toast>,
    /// Set once the user has asked to quit; the event loop checks this.
    pub should_quit: bool,
}

impl App {
    /// Build a fresh app sitting on a loading Home screen.
    pub fn new() -> Self {
        Self {
            screen: Screen::Home(HomeState::loading()),
            toast: None,
            should_quit: false,
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// State backing [`super::Screen::Canvas`].
///
/// Holds the loaded graph + layout as-is from the server, plus a
/// per-screen selection / viewport / status. Positions are read
/// from `layout.positions` directly; missing positions are filled
/// by the view layer's `auto_layout` call.
#[derive(Debug, Default)]
pub struct CanvasState {
    /// Semantic graph (source of truth).
    pub graph: Graph,
    /// Presentation overlay (positions + theme).
    pub layout: Layout,
    /// Currently selected node id, if any.
    pub selected: Option<NodeId>,
    /// `true` while the canvas HTTP fetch is in flight; the view
    /// shows a spinner instead of boxes.
    pub loading: bool,
}

impl CanvasState {
    /// A Canvas screen in its initial loading state, before the
    /// HTTP fetch returns.
    pub fn loading() -> Self {
        Self {
            graph: Graph::default(),
            layout: Layout::default(),
            selected: None,
            loading: true,
        }
    }
}

/// The set of screens the TUI can show. Data lives inside each variant so
/// illegal states (e.g. a session view with no session) are unrepresentable.
#[derive(Debug)]
pub enum Screen {
    /// Session list / launcher.
    Home(HomeState),
    /// New-session creation form.
    NewSession(NewSessionState),
    /// An open chat session.
    Session(SessionState),
    /// Architecture canvas: graph + layout read-only render.
    Canvas(CanvasState),
}

/// Severity of a [`Toast`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    /// Informational message.
    Info,
    /// Error message.
    Error,
}

/// A transient status message shown to the user.
// `Instant` is not `PartialEq`, so this only derives `Debug, Clone`.
#[derive(Debug, Clone)]
pub struct Toast {
    /// Message body.
    pub text: String,
    /// Whether this is an info or error toast.
    pub kind: ToastKind,
    /// When the toast was raised (for the fade-out animation).
    pub started_at: Instant,
}

impl Toast {
    /// Build an error toast.
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: ToastKind::Error,
            started_at: Instant::now(),
        }
    }

    /// Build an info toast.
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: ToastKind::Info,
            started_at: Instant::now(),
        }
    }
}
