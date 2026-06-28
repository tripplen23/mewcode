//! Update arm for [`Screen::Workspace`](super::super::model::Screen::Workspace).
//!
//! The Workspace is the doc-faithful unified screen from
//! `ui-aesthetic.md` §3. It absorbs both the chat (formerly
//! `Screen::Session`) and the canvas (formerly `Screen::Canvas`) into a
//! single screen with two regions. Mouse + key events route to the
//! focused region; `Tab` cycles focus.
//!
//! This file is the only update path for the Workspace — there is no
//! `update/canvas.rs` or `update/session.rs` anymore (T5 absorbed
//! them). T5 navigation primitives (hit-test, drag-pan, arrow keys)
//! live here as `pub(super)` helpers so the workspace tests can
//! exercise them directly.

use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use mewcode_protocol::canvas::{NodeId, Point};
use mewcode_protocol::event::ChatRequest;
use mewcode_protocol::{Message, MessagePart};
use tui_textarea::TextArea;

use super::super::model::{
    CanvasData, CanvasState, Cmd, HomeState, Screen, SessionState, Toast, WorkspaceFocus,
    WorkspaceState,
};
use super::key_to_input;

/// Pan stride (cells) for a single scroll-wheel tick.
const SCROLL_PAN: i32 = 3;

/// Cardinal direction for arrow-key selection. Pure data, no
/// crossterm dependency, so the unit tests don't need an event
/// source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    fn from_key(code: KeyCode) -> Option<Self> {
        match code {
            KeyCode::Up => Some(Direction::Up),
            KeyCode::Down => Some(Direction::Down),
            KeyCode::Left => Some(Direction::Left),
            KeyCode::Right => Some(Direction::Right),
            _ => None,
        }
    }
}

/// Handle a key event when the user is on the Workspace.
///
/// T5 spec, expanded for the unified screen:
/// - `Esc` returns the user to Home and refetches the session list.
/// - `Tab` cycles focus between the canvas and chat regions.
/// - Arrow keys: if focus is Canvas, move the selection to the
///   nearest node in that direction. If focus is Chat, scroll
///   the transcript (Up/Down) or move the cursor in the
///   prompt editor (Left/Right).
/// - Any other key goes to the chat prompt if a session
///   exists (Warp-style "type anywhere"), regardless of
///   focus. `Enter` submits; `/tools` and `/skills` open
///   overlays.
pub(super) fn on_workspace_key(
    screen: &mut Screen,
    toast: &mut Option<Toast>,
    key: KeyEvent,
) -> Cmd {
    let Screen::Workspace(ws) = screen else {
        return Cmd::None;
    };

    if key.code == KeyCode::Esc {
        // First Esc closes any open overlay in the chat region;
        // second Esc returns to Home. Mirrors the old Session
        // behaviour.
        if ws.focus == WorkspaceFocus::Chat {
            if let Some(s) = ws.chat.as_mut() {
                if s.overlay != super::super::model::Overlay::None {
                    s.overlay = super::super::model::Overlay::None;
                    return Cmd::None;
                }
            }
        }
        *screen = Screen::Home(HomeState::loading());
        return Cmd::LoadSessions;
    }

    if key.code == KeyCode::Tab {
        ws.cycle_focus();
        return Cmd::None;
    }

    // Key routing model:
    // - Arrow keys: if focus is Canvas, move selection. If
    //   focus is Chat, scroll the transcript (handled by
    //   `on_chat_key`).
    // - All other keys go to the chat input if a session
    //   exists. This matches Warp's "type anywhere, the
    //   prompt catches it" feel and means the user doesn't
    //   have to press Tab to start typing.
    // - Tab cycles focus (handled above).
    if Direction::from_key(key.code).is_some() {
        if ws.focus == WorkspaceFocus::Canvas {
            if let Some(dir) = Direction::from_key(key.code) {
                move_selection(&mut ws.canvas, dir);
            }
            return Cmd::None;
        }
        // Chat focus: let the chat handler do its thing
        // (Up/Down scroll, others go to the prompt editor).
        if let Some(s) = ws.chat.as_mut() {
            return on_chat_key(s, toast, key);
        }
        return Cmd::None;
    }

    if let Some(s) = ws.chat.as_mut() {
        return on_chat_key(s, toast, key);
    }

    Cmd::None
}

/// Handle a mouse event when the user is on the Workspace.
///
/// Mouse events land in the canvas region (the chat region is a
/// vertical strip at the bottom and gets the keyboard; the canvas
/// is the wide, clickable area on top). T5 nav primitives apply:
/// click to select, drag to pan, scroll to pan, arrows to move.
pub(super) fn on_workspace_mouse(screen: &mut Screen, mouse: MouseEvent) -> Cmd {
    let Screen::Workspace(ws) = screen else {
        return Cmd::None;
    };
    on_canvas_mouse(&mut ws.canvas, mouse)
}

/// Handle a key in the chat region: slash commands, submit, scroll,
/// overlay. Lifted from the old `update/session.rs`.
fn on_chat_key(s: &mut SessionState, toast: &mut Option<Toast>, key: KeyEvent) -> Cmd {
    match key.code {
        KeyCode::Enter => on_chat_submit(s, toast),
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

/// Move the transcript scroll offset by `delta` wrapped lines,
/// clamping into `[0, max_scroll]`. Reaching the bottom re-engages
/// auto-follow.
fn scroll_by(s: &mut SessionState, delta: i32) {
    let next = (s.scroll as i32 + delta).clamp(0, s.max_scroll as i32) as u16;
    s.scroll = next;
    s.follow = next >= s.max_scroll;
}

/// Handle `Enter` in the chat input. Three cases:
/// 1. Empty input → no-op.
/// 2. Streaming in flight → no-op (one turn at a time).
/// 3. Slash command → open the corresponding overlay.
/// 4. Otherwise → start a chat turn.
fn on_chat_submit(s: &mut SessionState, toast: &mut Option<Toast>) -> Cmd {
    let text = s.input.lines().join("\n");
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return Cmd::None;
    }
    if s.streaming.is_some() {
        return Cmd::None;
    }

    if let Some(rest) = trimmed.strip_prefix('/') {
        s.input = TextArea::default();
        match rest.split_whitespace().next().unwrap_or("") {
            "tools" => s.overlay = super::super::model::Overlay::Tools,
            "skills" => s.overlay = super::super::model::Overlay::Skills,
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
    s.follow = true;
    s.streaming = Some(super::super::model::StreamingState::new(uuid::Uuid::nil()));
    Cmd::StartChat(ChatRequest {
        session_id: s.session.id,
        model: s.session.model,
        mode: s.session.mode,
        messages: s.session.messages.clone(),
    })
}

/// Public submit: called by the auto-create path (a session was
/// just created from a pending prompt) and by the chat's `Enter`
/// handler (which has its own in-place submit). Takes a
/// pre-drained `text` so the caller controls the source.
pub(super) fn workspace_submit(
    s: &mut SessionState,
    toast: &mut Option<Toast>,
    text: String,
) -> Cmd {
    if s.streaming.is_some() {
        return Cmd::None;
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Cmd::None;
    }
    let user_msg = Message::user(vec![MessagePart::Text {
        text: trimmed.to_string(),
    }]);
    s.session.messages.push(user_msg);
    s.follow = true;
    s.streaming = Some(super::super::model::StreamingState::new(uuid::Uuid::nil()));
    let _ = toast; // suppress unused warning if no toast is set
    Cmd::StartChat(ChatRequest {
        session_id: s.session.id,
        model: s.session.model,
        mode: s.session.mode,
        messages: s.session.messages.clone(),
    })
}

/// Apply a finished `Msg::CanvasLoaded` to the Workspace. Populates
/// the canvas graph + layout on success, surfaces a toast on
/// failure, and clears the loading flag in either case. Selection
/// is cleared because the previous selection's node may have been
/// removed.
pub(super) fn apply_canvas_loaded(
    ws: &mut WorkspaceState,
    toast: &mut Option<Toast>,
    result: Result<CanvasData, String>,
) {
    ws.canvas.loading = false;
    match result {
        Ok(data) => {
            ws.canvas.graph = data.graph;
            ws.canvas.layout = data.layout;
            ws.canvas.selected = None;
        }
        Err(e) => {
            *toast = Some(Toast::error(format!("canvas load failed: {e}")));
        }
    }
}

// --- T5 navigation primitives on CanvasState (lifted from old update/canvas.rs) ---

/// Handle a mouse event on the canvas region.
pub(super) fn on_canvas_mouse(c: &mut CanvasState, mouse: MouseEvent) -> Cmd {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some(id) = hit_test(c, mouse.column, mouse.row) {
                c.selected = Some(id);
            }
            c.drag_origin = Some((mouse.column, mouse.row));
            Cmd::None
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if let Some((ox, oy)) = c.drag_origin {
                let dx = mouse.column as i32 - ox as i32;
                let dy = mouse.row as i32 - oy as i32;
                c.viewport.0 = c.viewport.0.saturating_add(dx);
                c.viewport.1 = c.viewport.1.saturating_add(dy);
                c.drag_origin = Some((mouse.column, mouse.row));
            }
            Cmd::None
        }
        MouseEventKind::Up(MouseButton::Left) => {
            c.drag_origin = None;
            Cmd::None
        }
        MouseEventKind::ScrollUp => {
            c.viewport.1 = c.viewport.1.saturating_add(SCROLL_PAN);
            Cmd::None
        }
        MouseEventKind::ScrollDown => {
            c.viewport.1 = c.viewport.1.saturating_sub(SCROLL_PAN);
            Cmd::None
        }
        MouseEventKind::ScrollLeft => {
            c.viewport.0 = c.viewport.0.saturating_add(SCROLL_PAN);
            Cmd::None
        }
        MouseEventKind::ScrollRight => {
            c.viewport.0 = c.viewport.0.saturating_sub(SCROLL_PAN);
            Cmd::None
        }
        _ => Cmd::None,
    }
}

/// Hit-test a click at view-coord `(col, row)`. Returns the id
/// of the topmost (last-drawn) node whose rect contains the
/// point, or `None`.
pub fn hit_test(c: &CanvasState, col: u16, row: u16) -> Option<NodeId> {
    let positions = c.resolved_positions();
    for node in &c.graph.nodes {
        if let Some(&p) = positions.get(&node.id) {
            let x0 = p.x.saturating_sub(c.viewport.0);
            let y0 = p.y.saturating_sub(c.viewport.1);
            let x1 = x0 + CanvasState::NODE_W;
            let y1 = y0 + CanvasState::NODE_H;
            if (x0 as u16) <= col && col < (x1 as u16) && (y0 as u16) <= row && row < (y1 as u16) {
                return Some(node.id.clone());
            }
        }
    }
    None
}

/// Move the selection to the nearest node in `dir`.
pub(crate) fn move_selection(c: &mut CanvasState, dir: Direction) {
    let positions = c.resolved_positions();
    let origin = c
        .selected
        .as_ref()
        .and_then(|id| positions.get(id).copied())
        .unwrap_or(Point { x: 0, y: 0 });
    let next = nearest_in_direction(&origin, dir, &positions, c.selected.as_ref());
    if let Some(id) = next {
        c.selected = Some(id);
    }
}

/// Pure nearest-in-direction helper.
pub fn nearest_in_direction(
    origin: &Point,
    dir: Direction,
    positions: &HashMap<NodeId, Point>,
    exclude: Option<&NodeId>,
) -> Option<NodeId> {
    let mut best: Option<(NodeId, i64)> = None;
    for (id, p) in positions {
        if exclude.is_some_and(|e| e == id) {
            continue;
        }
        let dx = (p.x - origin.x) as i64;
        let dy = (p.y - origin.y) as i64;
        let in_half_plane = match dir {
            Direction::Up => dy < 0,
            Direction::Down => dy > 0,
            Direction::Left => dx < 0,
            Direction::Right => dx > 0,
        };
        if !in_half_plane {
            continue;
        }
        let score = match dir {
            Direction::Up | Direction::Down => dy.abs() * 1000 + dx.abs(),
            Direction::Left | Direction::Right => dx.abs() * 1000 + dy.abs(),
        };
        if let Some((_, best_score)) = &best {
            if score < *best_score {
                best = Some((id.clone(), score));
            }
        } else {
            best = Some((id.clone(), score));
        }
    }
    best.map(|(id, _)| id)
}
