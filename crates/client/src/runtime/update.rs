//! The pure, synchronous Elm-style `update` function.
//!
//! > Idiom: the Elm update loop. `update` takes `&mut App` and a [`Msg`],
//! > mutates the model in place, and returns a [`Cmd`] describing any side
//! > effect. It performs **no I/O and no `.await`** — all async work happens in
//! > the loop's `Cmd` executor, whose results come back as more `Msg`s. Because
//! > the model is never borrowed across an `.await`, the borrow checker stays
//! > quiet and the function is trivially unit-testable.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_textarea::{Input, Key, TextArea};
use uuid::Uuid;

use crate::net::CreateSessionRequest;
use mewcode_protocol::event::ChatRequest;
use mewcode_protocol::{Message, MessagePart, Mode, ModelId, ToolCall, ToolResult};

use super::app::{
    App, Cmd, HomeState, Msg, NewSessionField, NewSessionState, Overlay, Screen, SessionState,
    StreamMsg, StreamingState, Toast, ToolCallView,
};

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
            Screen::Session(_) => on_session_key(screen, toast, key),
        },

        Msg::Tick => Cmd::None,

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

        Msg::SessionCreated(result) => match result {
            Ok(session) => {
                *screen = Screen::Session(SessionState::new(session));
                Cmd::None
            }
            Err(e) => {
                if let Screen::NewSession(n) = screen {
                    n.field = NewSessionField::Title;
                }
                *toast = Some(Toast::error(e));
                Cmd::None
            }
        },

        Msg::SessionOpened(result) => match result {
            Ok(session) => {
                *screen = Screen::Session(SessionState::new(session));
                Cmd::None
            }
            Err(e) => {
                *toast = Some(Toast::error(e));
                Cmd::None
            }
        },

        Msg::Stream(ev) => {
            if let Screen::Session(s) = screen {
                if let Some(t) = apply_stream_event(s, ev) {
                    *toast = Some(t);
                }
            }
            Cmd::None
        }
    }
}

/// Home screen: list navigation and the transitions out of it.
///
/// `Up`/`Down` clamp at the ends without wrapping; `Enter` on an empty list is a no-op.
fn on_home_key(screen: &mut Screen, should_quit: &mut bool, key: KeyEvent) -> Cmd {
    match key.code {
        KeyCode::Char('q') => {
            *should_quit = true;
            Cmd::None
        }
        KeyCode::Char('n') => {
            *screen = Screen::NewSession(NewSessionState::default());
            Cmd::None
        }
        KeyCode::Up => {
            if let Screen::Home(h) = screen {
                if !h.sessions.is_empty() {
                    h.selected = h.selected.saturating_sub(1);
                }
            }
            Cmd::None
        }
        KeyCode::Down => {
            if let Screen::Home(h) = screen {
                if !h.sessions.is_empty() {
                    h.selected = (h.selected + 1).min(h.sessions.len() - 1);
                }
            }
            Cmd::None
        }
        KeyCode::Enter => {
            if let Screen::Home(h) = screen {
                if let Some(summary) = h.sessions.get(h.selected) {
                    return Cmd::OpenSession(summary.id);
                }
            }
            Cmd::None
        }
        _ => Cmd::None,
    }
}

/// NewSession screen: field cycling, picker changes, validation, and submit.
fn on_new_session_key(screen: &mut Screen, toast: &mut Option<Toast>, key: KeyEvent) -> Cmd {
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
            let title = n.title.lines().join("\n");
            let trimmed = title.trim();
            if trimmed.is_empty() {
                n.field = NewSessionField::Title;
                *toast = Some(Toast::error("a non-empty title is required"));
                Cmd::None
            } else {
                let model = ModelId::ALL.get(n.model_idx).copied().unwrap_or_default();
                Cmd::CreateSession(CreateSessionRequest {
                    title: trimmed.to_string(),
                    model: Some(model),
                    mode: Some(n.mode),
                })
            }
        }
        KeyCode::Left => {
            match n.field {
                NewSessionField::Model => n.model_idx = n.model_idx.saturating_sub(1),
                NewSessionField::Mode => n.mode = toggle_mode(n.mode),
                // Pass Left through so the TextArea can move the cursor.
                NewSessionField::Title => { n.title.input(key_to_input(key)); }
            }
            Cmd::None
        }
        KeyCode::Right => {
            match n.field {
                NewSessionField::Model => {
                    n.model_idx = (n.model_idx + 1).min(ModelId::ALL.len().saturating_sub(1))
                }
                NewSessionField::Mode => n.mode = toggle_mode(n.mode),
                // Pass Right through so the TextArea can move the cursor.
                NewSessionField::Title => { n.title.input(key_to_input(key)); }
            }
            Cmd::None
        }
        _ => {
            if n.field == NewSessionField::Title {
                n.title.input(key_to_input(key));
            }
            Cmd::None
        }
    }
}

/// Session screen: input editing, submit, slash commands, and back-navigation.
fn on_session_key(screen: &mut Screen, toast: &mut Option<Toast>, key: KeyEvent) -> Cmd {
    let Screen::Session(s) = screen else {
        return Cmd::None;
    };

    if key.code == KeyCode::Esc {
        // Close an open overlay first; only leave the session on a second Esc.
        if s.overlay != Overlay::None {
            s.overlay = Overlay::None;
            return Cmd::None;
        }
        *screen = Screen::Home(HomeState::loading());
        return Cmd::LoadSessions;
    }

    match key.code {
        KeyCode::Enter => on_session_submit(s, toast),
        _ => {
            s.input.input(key_to_input(key));
            Cmd::None
        }
    }
}

/// Handle `Enter` in the Session input bar: slash command, send, or no-op.
fn on_session_submit(s: &mut SessionState, toast: &mut Option<Toast>) -> Cmd {
    let text = s.input.lines().join("\n");
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return Cmd::None;
    }

    // one turn at a time — refuse to start another while a turn is
    // in flight, leaving the input intact for the user to retry.
    if s.streaming.is_some() {
        return Cmd::None;
    }

    if let Some(rest) = trimmed.strip_prefix('/') {
        s.input = TextArea::default();
        match rest.split_whitespace().next().unwrap_or("") {
            "tools" => s.overlay = Overlay::Tools,
            "skills" => s.overlay = Overlay::Skills,
            other => {
                *toast = Some(Toast::error(format!("unknown command: /{other}")));
            }
        }
        return Cmd::None;
    }

    s.input = TextArea::default();
    let user_msg = Message::user(vec![MessagePart::Text {
        text: trimmed.to_string(),
    }]);
    s.session.messages.push(user_msg);
    // `Uuid::nil()` here is intentional: the real id arrives with the SSE
    // `Started` event; we need a placeholder so the `StreamingState` is Some.
    s.streaming = Some(StreamingState::new(Uuid::nil()));
    Cmd::StartChat(ChatRequest {
        session_id: s.session.id,
        model: s.session.model,
        mode: s.session.mode,
        messages: s.session.messages.clone(),
    })
}

/// Fold one SSE sub-message into the in-flight turn.
///
/// Returns `Some(Toast)` to raise on terminal failure, otherwise `None`. Events
/// that arrive with no [`StreamingState`] are ignored. On `Finished` exactly
/// one assistant message is committed and `streaming` returns to `None`; on
/// `Failed` the partial buffer is discarded and history is kept.
fn apply_stream_event(s: &mut SessionState, ev: StreamMsg) -> Option<Toast> {
    match ev {
        StreamMsg::Started(id) => {
            if let Some(st) = &mut s.streaming {
                st.assistant_id = id;
            }
            None
        }
        StreamMsg::Delta(delta) => {
            if let Some(st) = &mut s.streaming {
                st.buffer.push_str(&delta);
            }
            None
        }
        StreamMsg::ToolInput { id, name, input } => {
            if let Some(st) = &mut s.streaming {
                st.tool_calls.push(ToolCallView {
                    id,
                    name,
                    input,
                    output: None,
                });
            }
            None
        }
        StreamMsg::ToolOutput { id, output } => {
            if let Some(st) = &mut s.streaming {
                if let Some(call) = st.tool_calls.iter_mut().find(|c| c.id == id) {
                    call.output = Some(output);
                }
            }
            None
        }
        StreamMsg::Finished { .. } => {
            if let Some(st) = s.streaming.take() {
                let model = s.session.model;
                s.session.messages.push(commit_assistant_message(st, model));
            }
            None
        }
        StreamMsg::Failed(e) => {
            // Only react to a failure for a turn we are actually tracking.
            if s.streaming.take().is_some() {
                Some(Toast::error(format!("stream failed: {e}")))
            } else {
                None
            }
        }
    }
}

/// Assemble the committed assistant message from the streaming buffer and tool
/// calls. Text comes first, then each tool call followed by its output, so the
/// arrival order of tool parts is preserved.
fn commit_assistant_message(st: StreamingState, model: ModelId) -> Message {
    let mut parts: Vec<MessagePart> = Vec::new();
    if !st.buffer.is_empty() {
        parts.push(MessagePart::Text { text: st.buffer });
    }
    for call in st.tool_calls {
        let ToolCallView {
            id,
            name,
            input,
            output,
        } = call;
        parts.push(MessagePart::ToolCall(ToolCall {
            id: id.clone(),
            name: name.clone(),
            input,
        }));
        if let Some(output) = output {
            parts.push(MessagePart::ToolResult(ToolResult {
                call_id: id,
                name,
                output,
                is_error: false,
            }));
        }
    }
    Message::assistant(parts, model.provider_id())
}

/// Toggle between the two interaction modes.
fn toggle_mode(mode: Mode) -> Mode {
    match mode {
        Mode::Build => Mode::Plan,
        Mode::Plan => Mode::Build,
    }
}

/// Translate a crossterm [`KeyEvent`] into a [`tui_textarea::Input`].
///
/// tui-textarea 0.7 still bundles crossterm 0.28, so its built-in
/// `From<KeyEvent>` impl targets that older crate. The client talks crossterm
/// 0.29 (via ratatui 0.30), so we map the event ourselves — mirroring
/// tui-textarea's own mapping. Ceiling: this must stay in sync with
/// tui-textarea's conversion; upgrade path is deleting it once tui-textarea
/// publishes a crossterm-0.29 release. Key-release events are filtered by the
/// input reader upstream (`runtime::mod`), so this fn never sees them.
fn key_to_input(key: KeyEvent) -> Input {
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
