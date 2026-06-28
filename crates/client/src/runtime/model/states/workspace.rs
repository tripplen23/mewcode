//! The Workspace screen — the doc-faithful unified chat + canvas view.
//!
//! Per `docs/architecture-canvas/ui-aesthetic.md` §3, the Workspace shows
//! the canvas (palette | canvas | inspector panes) with a docked
//! block-strip + prompt editor at the bottom. M1 ships the shape
//! minimally (see the cutlines in T5 PR): the left rail is a slim
//! summary, the inspector is a single status row. The chat region is
//! always present, starts empty, and a session is auto-created on the
//! first submit (or `Cmd::PickSession` can attach an existing one).
//!
//! `ponytail:` this is one screen with two regions, not two screens
//! glued together. There is no `Screen::Canvas` and no
//! `Screen::Session` in M1 — both are absorbed here. Mouse + key events
//! route to the focused region; `Tab` cycles focus.

use mewcode_protocol::canvas::{Graph, Layout, NodeId, Point};
use tui_textarea::TextArea;

use crate::net::Session;

use super::session::SessionState;

/// The unified workspace: canvas + chat, always together.
///
/// The chat is `Option<SessionState>` because a session is created on
/// first submit. Until then, the prompt editor is visible but the
/// transcript area shows a "no session yet" hint.
#[derive(Debug)]
pub struct WorkspaceState {
    /// The canvas region (graph, layout, viewport, selection).
    pub canvas: CanvasState,
    /// The chat region (`None` until the first prompt submit).
    pub chat: Option<SessionState>,
    /// The currently focused region. `Tab` cycles between the two.
    pub focus: WorkspaceFocus,
    /// Pending prompt text: kept here when the chat is `None` and a
    /// submit lands, so the auto-created session receives it.
    pub pending_prompt: Option<String>,
}

impl WorkspaceState {
    /// Build a Workspace with an empty chat. The canvas starts in its
    /// loading state — the caller is expected to fire `Cmd::LoadCanvas`.
    pub fn loading_canvas() -> Self {
        Self {
            canvas: CanvasState::loading(),
            chat: None,
            focus: WorkspaceFocus::Canvas,
            pending_prompt: None,
        }
    }

    /// Build a Workspace with a session already attached to the chat
    /// region. The canvas starts in its loading state. Test helper
    /// (the production path uses `loading_canvas` + a separate
    /// `attach_session` call).
    pub fn with_session(s: SessionState) -> Self {
        Self {
            canvas: CanvasState::loading(),
            chat: Some(s),
            focus: WorkspaceFocus::Chat,
            pending_prompt: None,
        }
    }

    /// Cycle the focused region: Canvas → Chat → Canvas.
    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            WorkspaceFocus::Canvas => WorkspaceFocus::Chat,
            WorkspaceFocus::Chat => WorkspaceFocus::Canvas,
        };
    }
}

/// Which region of the Workspace currently receives keys and mouse
/// events. Mouse clicks on the *other* region are routed there
/// automatically (clicking in the chat region focuses the chat, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceFocus {
    /// Canvas region: arrow keys move selection, drag pans viewport.
    Canvas,
    /// Chat region: keys go to the prompt editor, the block strip
    /// scrolls.
    Chat,
}

/// State backing the canvas region of the Workspace.
///
/// Carries the loaded graph + layout, plus per-screen selection /
/// viewport / drag / status. Positions are read from
/// `layout.positions` directly; missing positions are filled by the
/// view layer's `ensure_resolved` call (a row-major grid that matches
/// the engine's `auto_layout`).
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
    /// Pan offset of the viewport, in graph coords. `(0, 0)` = no pan;
    /// positive `x` shifts the canvas content left (revealing the right
    /// edge), positive `y` shifts content up.
    pub viewport: (i32, i32),
    /// `Some(col, row)` while the user is mid-drag; `None` otherwise.
    pub drag_origin: Option<(u16, u16)>,
}

impl CanvasState {
    /// A fresh canvas in its initial loading state.
    pub fn loading() -> Self {
        Self {
            graph: Graph::default(),
            layout: Layout::default(),
            selected: None,
            loading: true,
            viewport: (0, 0),
            drag_origin: None,
        }
    }

    /// Width and height of a node card, in cell units. T5 navigation
    /// (hit-test, render) uses these constants.
    pub const NODE_W: i32 = 20;
    pub const NODE_H: i32 = 4;

    /// Resolve every node to a graph-coord `Point`, filling missing
    /// positions with a row-major grid that matches the engine's
    /// `auto_layout`. Used by both the view and the hit-test.
    pub fn resolved_positions(&self) -> std::collections::HashMap<NodeId, Point> {
        let mut sorted_ids: Vec<&NodeId> = self.graph.nodes.iter().map(|n| &n.id).collect();
        sorted_ids.sort_by(|a, b| a.0.cmp(&b.0));
        const COL_STEP: i32 = 24;
        const ROW_STEP: i32 = 6;
        const COLS_PER_ROW: usize = 4;
        let mut resolved: std::collections::HashMap<NodeId, Point> = self.layout.positions.clone();
        for (i, id) in sorted_ids.into_iter().enumerate() {
            if resolved.contains_key(id) {
                continue;
            }
            let col = (i % COLS_PER_ROW) as i32;
            let row = (i / COLS_PER_ROW) as i32;
            resolved.insert(
                id.clone(),
                Point {
                    x: col * COL_STEP,
                    y: row * ROW_STEP,
                },
            );
        }
        resolved
    }
}

/// Helper: attach an already-hydrated `Session` to the Workspace,
/// creating the chat region. Used by `Cmd::PickSession` and by
/// the auto-create path. Leaves the focus unchanged so the user
/// stays where they were (typically Canvas — attaching a
/// session shouldn't yank the user into the chat input).
pub fn attach_session(ws: &mut WorkspaceState, session: Session) {
    ws.chat = Some(SessionState::new(session));
    ws.pending_prompt = None;
}

/// Helper: take the pending prompt (if any) and the current
/// `TextArea` lines, returning the merged text. Clears the
/// textarea and pending slot. Used when auto-creating a session.
pub fn drain_prompt(ws: &mut WorkspaceState, textarea: &mut TextArea<'static>) -> String {
    let mut text = textarea.lines().join("\n");
    textarea.select_all();
    textarea.cut();
    if let Some(p) = ws.pending_prompt.take() {
        if !text.trim().is_empty() {
            text.push('\n');
        }
        text.push_str(&p);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loading_workspace_starts_with_canvas_focused() {
        let ws = WorkspaceState::loading_canvas();
        assert!(matches!(ws.focus, WorkspaceFocus::Canvas));
        assert!(ws.canvas.loading);
        assert!(ws.chat.is_none());
        assert!(ws.pending_prompt.is_none());
    }

    #[test]
    fn cycle_focus_bounces_between_regions() {
        let mut ws = WorkspaceState::loading_canvas();
        assert!(matches!(ws.focus, WorkspaceFocus::Canvas));
        ws.cycle_focus();
        assert!(matches!(ws.focus, WorkspaceFocus::Chat));
        ws.cycle_focus();
        assert!(matches!(ws.focus, WorkspaceFocus::Canvas));
    }

    #[test]
    fn resolved_positions_fills_missing() {
        // The existing T4/T5 tests cover the row-major math;
        // this just guards that the wrapper exists and works
        // on an empty graph.
        let c = CanvasState::loading();
        let m = c.resolved_positions();
        assert!(m.is_empty());
    }
}
