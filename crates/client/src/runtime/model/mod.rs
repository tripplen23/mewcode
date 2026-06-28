//! The application model for the Elm-style runtime.
//!
//! > Idiom: make illegal states unrepresentable. Each [`Screen`] variant owns
//! > its own state, so the workspace view ([`Screen::Workspace`]) can never
//! > exist without its canvas and (optionally) chat — the compiler
//! > guarantees it, and there are no `Option` fields to forget to populate.

mod cmd;
mod msg;
mod states;

pub use cmd::Cmd;
pub use msg::{CanvasData, CreateError, Msg, StreamMsg};
pub use states::{
    App, CanvasState, HomeState, ModelPicker, NewSessionField, NewSessionState, Overlay, Screen,
    SessionState, StreamingState, Toast, ToastKind, ToolCallView, WorkspaceFocus, WorkspaceState,
    attach_session, drain_prompt,
};
