//! The application's state types, grouped by where they live.
//!
//! `App` and `Screen` are the top-level state machine; `Toast` is a
//! cross-screen UI primitive. The remaining structs are split per screen
//! (matching the layout in [`super::super::view`] and [`super::super::update`])
//! so the file you open matches the file you'd change.
//!
//! M1's screen set is the doc-faithful `Workspace` (canvas + chat
//! docked together per `ui-aesthetic.md` §3), the launcher Home, and
//! the NewSession form. The old `Screen::Canvas` and `Screen::Session`
//! are gone — both are absorbed into `Workspace`.

use std::time::Instant;

mod home;
mod new_session;
mod session;
mod workspace;

pub use home::HomeState;
pub use new_session::{ModelPicker, NewSessionField, NewSessionState};
pub use session::{Overlay, SessionState, StreamingState, ToolCallView};
pub use workspace::{CanvasState, WorkspaceFocus, WorkspaceState, attach_session, drain_prompt};

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

/// The set of screens the TUI can show. Data lives inside each variant so
/// illegal states (e.g. a workspace without a project) are unrepresentable.
///
/// The `Workspace` variant is the largest by design — it carries the
/// canvas, the chat, and the focus state. Boxing it (per clippy's
/// `large_enum_variant` lint) would add an allocation on every
/// `Screen::Workspace` transition. For three screens in a TUI the
/// straight-line cost is fine; the extra indirection is not.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum Screen {
    /// Session list / launcher.
    Home(HomeState),
    /// New-session creation form.
    NewSession(NewSessionState),
    /// The unified workspace: canvas + chat, doc-faithful per
    /// `ui-aesthetic.md` §3.
    Workspace(WorkspaceState),
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
