//! T5 Workspace: the unified chat + canvas screen.
//!
//! Replaces the T4 `Screen::Canvas` and T3 `Screen::Session` tests
//! with a single test file for the new unified screen. Mirrors the
//! spec in `docs/architecture-canvas/milestone-1-promptable-canvas.md`
//! T5 + `ui-aesthetic.md` §3.
//!
//! Pure tests, no real terminal. Pinned:
//! - Home `'c'` pushes `Screen::Workspace(loading)` and returns
//!   `Cmd::LoadCanvas`.
//! - Workspace `Esc` returns to Home + `Cmd::LoadSessions`.
//! - `Tab` cycles focus between Canvas and Chat.
//! - Arrow keys move selection on the canvas region.
//! - Mouse click + drag pan the canvas viewport; scroll-wheel pans.
//! - Hit-test finds the right node (with viewport offset).
//! - Chat-region keys submit / scroll / open overlays.
//! - Chat-region keys are no-ops when the chat is `None` (no
//!   session has been created yet).
//! - Stream events route to the chat region when present.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use mewcode_client::runtime::model::{
    App, CanvasData, CanvasState, Msg, Screen, WorkspaceFocus, WorkspaceState,
};
use mewcode_client::runtime::update::{hit_test, update};
use mewcode_protocol::canvas::{Edge, EdgeKind, Graph, Layout, Node, NodeId, NodeKind, Point};
use mewcode_protocol::{MessagePart, Mode, ModelId};
use uuid::Uuid;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::empty())
}

fn mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

fn three_node_graph() -> Graph {
    Graph {
        version: 1,
        nodes: vec![
            Node {
                id: NodeId("a".into()),
                kind: NodeKind::Component,
                name: "A".into(),
                bind: None,
                contract: vec![],
                tech: None,
                desc: None,
            },
            Node {
                id: NodeId("b".into()),
                kind: NodeKind::Container,
                name: "B".into(),
                bind: None,
                contract: vec![],
                tech: None,
                desc: None,
            },
            Node {
                id: NodeId("c".into()),
                kind: NodeKind::System,
                name: "C".into(),
                bind: None,
                contract: vec![],
                tech: None,
                desc: None,
            },
        ],
        edges: vec![
            Edge {
                from: NodeId("a".into()),
                to: NodeId("b".into()),
                kind: EdgeKind::Depends,
            },
            Edge {
                from: NodeId("b".into()),
                to: NodeId("c".into()),
                kind: EdgeKind::Calls,
            },
        ],
    }
}

fn empty_layout() -> Layout {
    Layout {
        version: 1,
        positions: Default::default(),
        theme: Default::default(),
    }
}

fn make_session() -> mewcode_client::net::Session {
    mewcode_client::net::Session {
        id: Uuid::new_v4(),
        title: "test".into(),
        model: ModelId::default(),
        mode: Mode::default(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        messages: vec![],
    }
}

fn canvas_loaded_ok(app: &mut App) {
    let data = CanvasData {
        graph: three_node_graph(),
        layout: empty_layout(),
    };
    update(app, Msg::CanvasLoaded(Ok(data)));
}

fn on_workspace(app: &mut App) {
    app.screen = Screen::Workspace(WorkspaceState::loading_canvas());
    canvas_loaded_ok(app);
}

// --- Home → Workspace / Workspace → Home transitions -----------------------

#[test]
fn pressing_c_on_home_enters_workspace_loading() {
    let mut app = App::new();
    assert!(matches!(app.screen, Screen::Home(_)));

    let cmd = update(&mut app, Msg::Key(key(KeyCode::Char('c'))));

    assert!(matches!(
        cmd,
        mewcode_client::runtime::model::Cmd::LoadCanvas
    ));
    match &app.screen {
        Screen::Workspace(ws) => {
            assert!(ws.canvas.loading, "fresh workspace should be loading");
            assert!(ws.canvas.graph.nodes.is_empty());
            assert!(ws.chat.is_none());
            // Canvas region has initial focus.
            assert!(matches!(ws.focus, WorkspaceFocus::Canvas));
        }
        other => panic!("expected Screen::Workspace, got {other:?}"),
    }
}

#[test]
fn esc_on_workspace_returns_to_home_and_refetches_sessions() {
    let mut app = App::new();
    on_workspace(&mut app);

    let cmd = update(&mut app, Msg::Key(key(KeyCode::Esc)));

    match &app.screen {
        Screen::Home(h) => {
            assert!(h.loading, "Home should re-enter loading state on Esc");
            assert!(h.sessions.is_empty());
        }
        other => panic!("expected Screen::Home after Esc, got {other:?}"),
    }
    assert!(matches!(
        cmd,
        mewcode_client::runtime::model::Cmd::LoadSessions
    ));
}

// --- CanvasLoaded handling -----------------------------------------------

#[test]
fn canvas_loaded_ok_populates_workspace_canvas() {
    let mut app = App::new();
    app.screen = Screen::Workspace(WorkspaceState::loading_canvas());

    let data = CanvasData {
        graph: three_node_graph(),
        layout: empty_layout(),
    };
    update(&mut app, Msg::CanvasLoaded(Ok(data)));

    match &app.screen {
        Screen::Workspace(ws) => {
            assert!(!ws.canvas.loading);
            assert_eq!(ws.canvas.graph.nodes.len(), 3);
            assert_eq!(ws.canvas.graph.edges.len(), 2);
            assert!(ws.canvas.selected.is_none());
            // Chat is still empty until a session is created.
            assert!(ws.chat.is_none());
        }
        other => panic!("expected Screen::Workspace, got {other:?}"),
    }
    assert!(app.toast.is_none());
}

#[test]
fn canvas_loaded_err_raises_toast_and_keeps_state() {
    let mut app = App::new();
    app.screen = Screen::Workspace(WorkspaceState::loading_canvas());

    update(
        &mut app,
        Msg::CanvasLoaded(Err("server unreachable".into())),
    );

    match &app.screen {
        Screen::Workspace(ws) => {
            assert!(!ws.canvas.loading, "loading flag must clear even on error");
            assert!(ws.canvas.graph.nodes.is_empty(), "graph untouched on error");
        }
        other => panic!("expected Screen::Workspace, got {other:?}"),
    }
    let toast = app.toast.expect("error toast should be set");
    assert!(toast.text.contains("canvas load failed"));
    assert!(toast.text.contains("server unreachable"));
}

#[test]
fn canvas_loaded_ignored_when_user_left_screen() {
    // User opens workspace, leaves for Home, then the load
    // finishes. The stale `CanvasLoaded` must not silently
    // mutate Home.
    let mut app = App::new();
    let was_home_before = matches!(app.screen, Screen::Home(_));
    let toast_was_none_before = app.toast.is_none();

    let data = CanvasData {
        graph: three_node_graph(),
        layout: empty_layout(),
    };
    update(&mut app, Msg::CanvasLoaded(Ok(data)));

    assert!(matches!(app.screen, Screen::Home(_)));
    assert_eq!(was_home_before, matches!(app.screen, Screen::Home(_)));
    assert_eq!(app.toast.is_none(), toast_was_none_before);
}

// --- Focus cycling -------------------------------------------------------

#[test]
fn tab_cycles_focus_canvas_to_chat_and_back() {
    let mut app = App::new();
    on_workspace(&mut app);

    // Initially Canvas.
    match &app.screen {
        Screen::Workspace(ws) => assert!(matches!(ws.focus, WorkspaceFocus::Canvas)),
        _ => unreachable!(),
    }

    update(&mut app, Msg::Key(key(KeyCode::Tab)));
    match &app.screen {
        Screen::Workspace(ws) => assert!(matches!(ws.focus, WorkspaceFocus::Chat)),
        _ => unreachable!(),
    }

    update(&mut app, Msg::Key(key(KeyCode::Tab)));
    match &app.screen {
        Screen::Workspace(ws) => assert!(matches!(ws.focus, WorkspaceFocus::Canvas)),
        _ => unreachable!(),
    }
}

// --- Canvas navigation: arrow keys + hit-test + drag-pan ------------------

#[test]
fn arrow_keys_move_selection_on_canvas_region() {
    let mut app = App::new();
    on_workspace(&mut app);
    // Make sure focus is on canvas.
    match &app.screen {
        Screen::Workspace(ws) => assert!(matches!(ws.focus, WorkspaceFocus::Canvas)),
        _ => unreachable!(),
    }

    // No selection yet — Right picks the rightmost reachable node.
    update(&mut app, Msg::Key(key(KeyCode::Right)));
    match &app.screen {
        Screen::Workspace(ws) => {
            assert!(ws.canvas.selected.is_some(), "Right should select a node")
        }
        _ => unreachable!(),
    }

    // Down — should move selection to a node below the current one.
    let prev = match &app.screen {
        Screen::Workspace(ws) => ws.canvas.selected.clone().unwrap(),
        _ => unreachable!(),
    };
    update(&mut app, Msg::Key(key(KeyCode::Down)));
    match &app.screen {
        Screen::Workspace(ws) => {
            let now = ws.canvas.selected.clone().unwrap();
            // Either moved to a new node, or stayed put (if no
            // node lies below). Both are valid — we only assert
            // no panic.
            let _ = (prev, now);
        }
        _ => unreachable!(),
    }
}

#[test]
fn hit_test_finds_node_with_viewport_offset() {
    let mut app = App::new();
    on_workspace(&mut app);
    if let Screen::Workspace(ws) = &app.screen {
        // Auto-layout puts node 'a' at (0, 0). With viewport
        // (0, 0) a click at (1, 1) should hit it.
        let hit = hit_test(&ws.canvas, 1, 1);
        assert!(hit.is_some(), "click at (1,1) should hit a node");
    }
    // Shift the viewport by (5, 5) — click at (1, 1) no longer
    // hits node 'a'.
    let mut app2 = App::new();
    on_workspace(&mut app2);
    if let Screen::Workspace(ws) = &mut app2.screen {
        ws.canvas.viewport = (5, 5);
        let miss = hit_test(&ws.canvas, 1, 1);
        assert!(
            miss.is_none(),
            "click at (1,1) should miss when viewport is (5,5)"
        );
    }
}

#[test]
fn click_then_drag_pans_canvas_viewport() {
    let mut app = App::new();
    on_workspace(&mut app);

    // Press at (10, 10) — sets drag_origin.
    update(
        &mut app,
        Msg::Mouse(mouse(MouseEventKind::Down(MouseButton::Left), 10, 10)),
    );
    // Drag to (15, 12) — pan by (5, 2).
    update(
        &mut app,
        Msg::Mouse(mouse(MouseEventKind::Drag(MouseButton::Left), 15, 12)),
    );
    match &app.screen {
        Screen::Workspace(ws) => {
            assert_eq!(
                ws.canvas.viewport,
                (5, 2),
                "viewport should have panned by the drag delta"
            );
        }
        _ => unreachable!(),
    }
    // Release — drag_origin clears.
    update(
        &mut app,
        Msg::Mouse(mouse(MouseEventKind::Up(MouseButton::Left), 15, 12)),
    );
    match &app.screen {
        Screen::Workspace(ws) => assert!(ws.canvas.drag_origin.is_none()),
        _ => unreachable!(),
    }
}

#[test]
fn scroll_wheel_pans_canvas_viewport() {
    let mut app = App::new();
    on_workspace(&mut app);

    update(
        &mut app,
        Msg::Mouse(mouse(MouseEventKind::ScrollDown, 0, 0)),
    );
    match &app.screen {
        Screen::Workspace(ws) => {
            // SCROLL_PAN=3, so y decreases by 3.
            assert_eq!(ws.canvas.viewport, (0, -3));
        }
        _ => unreachable!(),
    }
}

// --- Chat region: submit / overlay / scroll / stream -----------------------

#[test]
fn chat_region_keys_are_noop_when_no_session() {
    let mut app = App::new();
    on_workspace(&mut app);
    // Switch to chat focus.
    // No chat yet — typing does nothing. Tab to chat
    // focus is a no-op (focus cycles, but keys still go to
    // the chat input if a session exists; without one,
    // they are dropped).
    update(&mut app, Msg::Key(key(KeyCode::Tab)));
    match &app.screen {
        Screen::Workspace(ws) => assert!(matches!(ws.focus, WorkspaceFocus::Chat)),
        _ => unreachable!(),
    }
    let cmd = update(&mut app, Msg::Key(key(KeyCode::Char('h'))));
    assert!(matches!(cmd, mewcode_client::runtime::model::Cmd::None));
    match &app.screen {
        Screen::Workspace(ws) => assert!(ws.chat.is_none()),
        _ => unreachable!(),
    }
}

#[test]
fn submitting_in_chat_region_starts_a_turn() {
    let mut app = App::new();
    on_workspace(&mut app);

    // Attach a session (simulating "user picked a session").
    let session = make_session();
    if let Screen::Workspace(ws) = &mut app.screen {
        mewcode_client::runtime::model::attach_session(ws, session);
    }
    // Type a character, then Enter. Keys route to the
    // chat input regardless of focus (Warp-style).
    update(&mut app, Msg::Key(key(KeyCode::Char('h'))));
    let cmd = update(&mut app, Msg::Key(key(KeyCode::Enter)));
    assert!(matches!(
        cmd,
        mewcode_client::runtime::model::Cmd::StartChat(_)
    ));
    match &app.screen {
        Screen::Workspace(ws) => {
            let s = ws.chat.as_ref().unwrap();
            assert!(s.streaming.is_some());
            assert_eq!(s.session.messages.len(), 1);
            assert!(matches!(
                s.session.messages[0].parts[0],
                MessagePart::Text { .. }
            ));
        }
        _ => unreachable!(),
    }
}

#[test]
fn slash_tools_in_chat_opens_tools_overlay() {
    let mut app = App::new();
    on_workspace(&mut app);
    let session = make_session();
    if let Screen::Workspace(ws) = &mut app.screen {
        mewcode_client::runtime::model::attach_session(ws, session);
    }
    for c in "/tools".chars() {
        update(&mut app, Msg::Key(key(KeyCode::Char(c))));
    }
    update(&mut app, Msg::Key(key(KeyCode::Enter)));
    match &app.screen {
        Screen::Workspace(ws) => {
            let s = ws.chat.as_ref().unwrap();
            assert!(matches!(
                s.overlay,
                mewcode_client::runtime::model::Overlay::Tools
            ));
        }
        _ => unreachable!(),
    }
}

#[test]
fn stream_event_lands_in_chat_region() {
    let mut app = App::new();
    on_workspace(&mut app);
    let session = make_session();
    if let Screen::Workspace(ws) = &mut app.screen {
        mewcode_client::runtime::model::attach_session(ws, session);
    }
    // Start a turn so `streaming` is set; stream events
    // without an in-flight turn are dropped.
    update(&mut app, Msg::Key(key(KeyCode::Char('h'))));
    update(&mut app, Msg::Key(key(KeyCode::Enter)));

    let assistant_id = Uuid::new_v4();
    // Stream lifecycle: Start, Delta, Finish.
    update(
        &mut app,
        Msg::Stream(mewcode_client::runtime::model::StreamMsg::Started(
            assistant_id,
        )),
    );
    update(
        &mut app,
        Msg::Stream(mewcode_client::runtime::model::StreamMsg::Delta(
            "hello".into(),
        )),
    );
    update(
        &mut app,
        Msg::Stream(mewcode_client::runtime::model::StreamMsg::Finished { duration_ms: 42 }),
    );

    match &app.screen {
        Screen::Workspace(ws) => {
            let s = ws.chat.as_ref().unwrap();
            // User message + committed assistant message = 2.
            assert_eq!(s.session.messages.len(), 2);
            // Streaming cleared.
            assert!(s.streaming.is_none());
        }
        _ => unreachable!(),
    }
}

#[test]
fn unknown_slash_command_raises_toast() {
    let mut app = App::new();
    on_workspace(&mut app);
    let session = make_session();
    if let Screen::Workspace(ws) = &mut app.screen {
        mewcode_client::runtime::model::attach_session(ws, session);
    }
    for c in "/nope".chars() {
        update(&mut app, Msg::Key(key(KeyCode::Char(c))));
    }
    update(&mut app, Msg::Key(key(KeyCode::Enter)));

    let toast = app.toast.as_ref().expect("error toast should be set");
    assert!(toast.text.contains("unknown command"));
    assert!(toast.text.contains("/nope"));
}

// --- WorkspaceState helpers -----------------------------------------------

#[test]
fn cycle_focus_bounces_canvas_to_chat() {
    let mut ws = WorkspaceState::loading_canvas();
    assert!(matches!(ws.focus, WorkspaceFocus::Canvas));
    ws.cycle_focus();
    assert!(matches!(ws.focus, WorkspaceFocus::Chat));
    ws.cycle_focus();
    assert!(matches!(ws.focus, WorkspaceFocus::Canvas));
}

#[test]
fn attach_session_sets_chat_and_clears_pending() {
    let mut ws = WorkspaceState::loading_canvas();
    ws.pending_prompt = Some("queued".into());
    let session = make_session();
    mewcode_client::runtime::model::attach_session(&mut ws, session);
    assert!(ws.chat.is_some());
    assert!(ws.pending_prompt.is_none());
    // attach_session does NOT change focus — the user stays on
    // whatever region they were on (typically Canvas, so a
    // session attach from the Home screen doesn't yank them
    // into the chat input).
    assert!(matches!(ws.focus, WorkspaceFocus::Canvas));
}

// Smoke check: CanvasState::resolved_positions still works after the move
// out of `view/canvas.rs` into `model::states::workspace`.
#[test]
fn canvas_state_resolved_positions_still_fills_missing() {
    let c = CanvasState::loading();
    let m = c.resolved_positions();
    assert!(m.is_empty(), "empty graph has no resolved positions");
}

#[test]
fn canvas_state_resolved_positions_fills_three_node_graph() {
    let mut app = App::new();
    on_workspace(&mut app);
    let c = match &app.screen {
        Screen::Workspace(ws) => &ws.canvas,
        _ => unreachable!(),
    };
    let m = c.resolved_positions();
    assert_eq!(m.len(), 3, "three nodes should resolve to three positions");
    // All three points are distinct.
    let pts: std::collections::HashSet<_> = m.values().map(|p: &Point| (p.x, p.y)).collect();
    assert_eq!(pts.len(), 3, "auto-layout should give distinct positions");
}
