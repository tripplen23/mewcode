use crossterm::event::{KeyCode, KeyEvent};
use tui_textarea::TextArea;
use uuid::Uuid;

use mewcode_protocol::event::ChatRequest;
use mewcode_protocol::{Message, MessagePart};

use super::super::model::{Cmd, HomeState, Overlay, Screen, SessionState, StreamingState, Toast};
use super::key_to_input;

/// Session screen: input editing, submit, slash commands, and back-navigation.
pub(super) fn on_session_key(screen: &mut Screen, toast: &mut Option<Toast>, key: KeyEvent) -> Cmd {
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
        // Transcript scrollback. Up/PageUp release auto-follow; scrolling back
        // to the bottom re-engages it. `max_scroll`/`viewport` come from the
        // last rendered frame (see `view::render_session`).
        KeyCode::Up => {
            scroll_by(s, -1);
            Cmd::None
        }
        KeyCode::Down => {
            scroll_by(s, 1);
            Cmd::None
        }
        KeyCode::PageUp => {
            scroll_by(s, -(s.viewport.max(1) as i32));
            Cmd::None
        }
        KeyCode::PageDown => {
            scroll_by(s, s.viewport.max(1) as i32);
            Cmd::None
        }
        _ => {
            s.input.input(key_to_input(key));
            Cmd::None
        }
    }
}

/// Move the transcript scroll offset by `delta` wrapped lines, clamping into
/// `[0, max_scroll]`. Scrolling up releases auto-follow; reaching the bottom
/// re-engages it so new replies keep scrolling into view.
fn scroll_by(s: &mut SessionState, delta: i32) {
    let next = (s.scroll as i32 + delta).clamp(0, s.max_scroll as i32) as u16;
    s.scroll = next;
    s.follow = next >= s.max_scroll;
}

/// Handle `Enter` in the Session input bar: slash command, send, or no-op.
pub(super) fn on_session_submit(s: &mut SessionState, toast: &mut Option<Toast>) -> Cmd {
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
    // Snap back to the latest line so the user watches the reply stream in.
    s.follow = true;
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
