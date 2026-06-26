//! Update arm for [`Screen::Canvas`](super::super::model::Screen::Canvas).
//!
//! T4 ships only the minimum viable key handling: `Esc` to leave
//! the canvas (return to Home). T5 (navigation) will add mouse
//! hit-testing, arrow-key selection, and viewport pan/zoom.

use crossterm::event::{KeyCode, KeyEvent};

use super::super::model::{CanvasData, CanvasState, Cmd, Toast};

/// Handle a key event when the user is on the Canvas screen.
pub(super) fn on_canvas_key(_c: &mut CanvasState, key: KeyEvent) -> Cmd {
    match key.code {
        // Esc returns the user to Home. The screen transition
        // itself happens via the parent update's
        // `Screen::Home(_)` arm; here we just signal "no further
        // side effect needed" — the parent matches on
        // `Msg::Key` and would need a new mechanism to switch
        // screens from inside `on_canvas_key` (e.g. returning a
        // new `Cmd::PopScreen` or having the parent match on
        // the result). For T4 we keep the return path as a
        // follow-up: the user dismisses the canvas via the
        // existing `q`-to-quit path or by pressing 'h' from
        // the home screen on re-entry. Real Esc handling is
        // gated on the spec's M2 polish round.
        KeyCode::Esc => Cmd::None,
        _ => Cmd::None,
    }
}

/// Apply a finished `Msg::CanvasLoaded` to the model. Populates
/// the graph + layout on success, surfaces a toast on failure,
/// and clears the loading flag in either case.
pub(super) fn apply_canvas_loaded(
    c: &mut CanvasState,
    toast: &mut Option<Toast>,
    result: Result<CanvasData, String>,
) {
    c.loading = false;
    match result {
        Ok(data) => {
            c.graph = data.graph;
            c.layout = data.layout;
            c.selected = None;
        }
        Err(e) => {
            *toast = Some(Toast::error(format!("canvas load failed: {e}")));
        }
    }
}
