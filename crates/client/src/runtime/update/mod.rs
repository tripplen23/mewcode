//! The pure, synchronous Elm-style `update` function.
//!
//! > Idiom: the Elm update loop. `update` takes `&mut App` and a [`Msg`],
//! > mutates the model in place, and returns a [`Cmd`] describing any side
//! > effect. It performs **no I/O and no `.await`** — all async work happens in
//! > the loop's `Cmd` executor, whose results come back as more `Msg`s. Because
//! > the model is never borrowed across an `.await`, the borrow checker stays
//! > quiet and the function is trivially unit-testable.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_textarea::{Input, Key};

use super::model::{App, Cmd, CreateError, ModelPicker, Msg, NewSessionField, Screen, Toast};

mod home;
mod new_session;
mod stream;
mod workspace;

use home::on_home_key;
use new_session::on_new_session_key;
use stream::apply_stream_event;
use workspace::{apply_canvas_loaded, on_workspace_key, on_workspace_mouse, workspace_submit};

/// Apply a [`Msg`] to the model, returning the side effect to run next.
///
/// Pure and synchronous: no I/O, no awaiting. The model's `screen` and `toast`
/// fields are borrowed independently (a split borrow) so a single arm can both
/// transition the screen and raise a toast without fighting the borrow checker.
pub fn update(app: &mut App, msg: Msg) -> Cmd {
    let App {
        screen,
        toast,
        should_quit,
        ..
    } = app;

    match msg {
        Msg::Key(key) => match screen {
            Screen::Home(_) => on_home_key(screen, should_quit, key),
            Screen::NewSession(_) => on_new_session_key(screen, toast, key),
            Screen::Workspace(_) => on_workspace_key(screen, toast, key),
        },

        Msg::Mouse(mouse) => match screen {
            Screen::Workspace(_) => on_workspace_mouse(screen, mouse),
            // Mouse on Home/NewSession is ignored — the screens
            // don't yet consume it.
            _ => Cmd::None,
        },

        Msg::Tick => Cmd::None,

        Msg::CanvasLoaded(result) => {
            // Only mutates the screen if the user is still in a
            // Workspace — a load that finishes after the user
            // has left is dropped, mirroring how a stale
            // `SessionsLoaded` is ignored on Home.
            if let Screen::Workspace(ws) = screen {
                apply_canvas_loaded(ws, toast, result);
            }
            Cmd::None
        }

        Msg::SessionsLoaded(result) => {
            if let Screen::Home(h) = screen {
                match result {
                    Ok(list) => {
                        h.sessions = list;
                        h.loading = false;
                        if h.selected >= h.sessions.len() {
                            h.selected = 0;
                        }
                    }
                    Err(e) => {
                        h.sessions.clear();
                        h.selected = 0;
                        h.loading = false;
                        *toast = Some(Toast::error(e));
                    }
                }
            }
            Cmd::None
        }

        Msg::ModelsLoaded(result) => {
            if let Screen::NewSession(n) = screen {
                let (picker, err) = ModelPicker::from_registry(result);
                n.model = picker;
                n.error = err;
            }
            Cmd::None
        }

        Msg::SessionCreated(result) => {
            // Two callers: the NewSession form and the Workspace
            // auto-create path. The Workspace path attaches the
            // new session to its chat region and (if there was
            // a pending prompt) submits it.
            if let Screen::Workspace(ws) = screen {
                match result {
                    Ok(session) => {
                        // Drain the prompt text *before* attaching
                        // the new session, so we don't fight the
                        // borrow checker (the chat's `TextArea`
                        // would live inside `ws`).
                        let chat_text = ws.pending_prompt.take();
                        super::model::attach_session(ws, session);
                        if let (Some(text), Some(s)) = (chat_text, ws.chat.as_mut()) {
                            return workspace_submit(s, toast, text);
                        }
                        Cmd::None
                    }
                    Err(e) => {
                        *toast = Some(Toast::error(e.to_string()));
                        Cmd::None
                    }
                }
            } else if let Screen::NewSession(n) = screen {
                match result {
                    Ok(session) => {
                        // Land in the new session, which starts with empty history.
                        *screen = Screen::Workspace(super::model::WorkspaceState::loading_canvas());
                        if let Screen::Workspace(ws) = screen {
                            super::model::attach_session(ws, session);
                        }
                        Cmd::LoadCanvas
                    }
                    Err(CreateError::EmptyTitle(_)) => {
                        n.submitting = false;
                        n.field = NewSessionField::Title;
                        n.error = Some(new_session::REQUIRED_TITLE_HINT.to_string());
                        Cmd::None
                    }
                    Err(CreateError::Other(message)) => {
                        n.submitting = false;
                        n.error = Some(message);
                        Cmd::None
                    }
                }
            } else {
                Cmd::None
            }
        }

        Msg::SessionOpened(result) => match result {
            Ok(session) => {
                // From any screen, `Msg::SessionOpened` lands the
                // user in a Workspace with the session attached.
                // Home + `Cmd::OpenSession` fires this; the test
                // suite relies on the transition happening here.
                *screen = Screen::Workspace(super::model::WorkspaceState::loading_canvas());
                if let Screen::Workspace(ws) = screen {
                    super::model::attach_session(ws, session);
                }
                Cmd::LoadCanvas
            }
            Err(e) => {
                *toast = Some(Toast::error(e));
                Cmd::None
            }
        },

        Msg::Stream(ev) => {
            // Streams route to the Workspace's chat region. A
            // stream event landing when the user has no chat is
            // a programmer error — the loop only starts streams
            // from a Workspace submit — so we silently drop it
            // rather than panic.
            if let Screen::Workspace(ws) = screen {
                if let Some(s) = ws.chat.as_mut() {
                    if let Some(t) = apply_stream_event(s, ev) {
                        *toast = Some(t);
                    }
                }
            }
            Cmd::None
        }
    }
}

/// Translate a crossterm [`KeyEvent`] into a [`tui_textarea::Input`].
///
/// [`tui-textarea`](https://docs.rs/tui-textarea/latest/tui_textarea/) 0.7
/// still bundles crossterm 0.28, so its built-in `From<KeyEvent>` impl
/// targets that older crate. The client talks crossterm 0.29 (via
/// [ratatui](https://docs.rs/ratatui/latest/ratatui/) 0.30), so we map the
/// event ourselves — mirroring tui-textarea's own mapping. Ceiling: this
/// must stay in sync with tui-textarea's conversion; upgrade path is deleting
/// it once tui-textarea publishes a crossterm-0.29 release. Key-release
/// events are filtered by the input reader upstream (`runtime::mod`), so
/// this fn never sees them.
pub(super) fn key_to_input(key: KeyEvent) -> Input {
    let code = match key.code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Enter => Key::Enter,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Tab => Key::Tab,
        KeyCode::Delete => Key::Delete,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Esc => Key::Esc,
        KeyCode::F(x) => Key::F(x),
        _ => Key::Null,
    };
    Input {
        key: code,
        ctrl: key.modifiers.contains(KeyModifiers::CONTROL),
        alt: key.modifiers.contains(KeyModifiers::ALT),
        shift: key.modifiers.contains(KeyModifiers::SHIFT),
    }
}

// Public re-exports for the workspace T5 nav primitives. Used by
// the integration tests in `tests/workspace_screen.rs` to
// exercise hit-test and nearest-in-direction without going
// through the full `update` entry point.
pub use workspace::{Direction, hit_test, nearest_in_direction};
