use crossterm::event::{KeyCode, KeyEvent};

use mewcode_protocol::Mode;

use crate::net::CreateSessionRequest;

use super::super::model::{Cmd, HomeState, NewSessionField, Screen, Toast};
use super::key_to_input;

/// Hint shown when the user tries to submit an empty/whitespace title.
///
/// Shared with the `SessionCreated(CreateError::EmptyTitle)` arm in
/// [`super`]'s `update` so the client- and server-side rejections read alike.
pub(super) const REQUIRED_TITLE_HINT: &str = "a non-empty title is required";

/// NewSession screen: field cycling, picker changes, validation, and submit.
pub(super) fn on_new_session_key(
    screen: &mut Screen,
    toast: &mut Option<Toast>,
    key: KeyEvent,
) -> Cmd {
    if key.code == KeyCode::Esc {
        *screen = Screen::Home(HomeState::loading());
        return Cmd::LoadSessions;
    }

    let Screen::NewSession(n) = screen else {
        return Cmd::None;
    };

    match key.code {
        KeyCode::Tab => {
            n.field = n.field.next();
            Cmd::None
        }
        KeyCode::Enter => {
            // In-flight guard: a submit is already running, so ignore this
            // press and start no second `POST /sessions`.
            if n.submitting {
                return Cmd::None;
            }
            let title = n.title.lines().join("\n");
            let trimmed = title.trim();
            if trimmed.is_empty() {
                n.field = NewSessionField::Title;
                n.error = Some(REQUIRED_TITLE_HINT.to_string());
                *toast = Some(Toast::error(REQUIRED_TITLE_HINT));
                Cmd::None
            } else {
                let model = n.model.selected_model().unwrap_or_default();
                let req = CreateSessionRequest {
                    title: trimmed.to_string(),
                    model: Some(model),
                    mode: Some(n.mode),
                };
                // Clear any stale error and mark the request in flight before
                // the loop dispatches it (the guard above relies on this).
                n.error = None;
                n.submitting = true;
                Cmd::CreateSession(req)
            }
        }
        KeyCode::Left => {
            match n.field {
                NewSessionField::Model => {
                    n.model.select_prev();
                    n.error = None;
                }
                NewSessionField::Mode => {
                    n.mode = toggle_mode(n.mode);
                    n.error = None;
                }
                // Pass Left through so the TextArea can move the cursor.
                NewSessionField::Title => {
                    n.title.input(key_to_input(key));
                }
            }
            Cmd::None
        }
        KeyCode::Right => {
            match n.field {
                NewSessionField::Model => {
                    n.model.select_next();
                    n.error = None;
                }
                NewSessionField::Mode => {
                    n.mode = toggle_mode(n.mode);
                    n.error = None;
                }
                // Pass Right through so the TextArea can move the cursor.
                NewSessionField::Title => {
                    n.title.input(key_to_input(key));
                }
            }
            Cmd::None
        }
        _ => {
            if n.field == NewSessionField::Title {
                n.title.input(key_to_input(key));
                n.error = None;
            }
            Cmd::None
        }
    }
}

/// Toggle between the two interaction modes.
fn toggle_mode(mode: Mode) -> Mode {
    match mode {
        Mode::Build => Mode::Plan,
        Mode::Plan => Mode::Build,
    }
}
