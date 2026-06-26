//! T3 mouse-capture regression: every screen must ignore `Msg::Mouse`
//! with no behaviour change. T5 (canvas navigation) will attach
//! real handlers in a follow-up PR; this test pins the "no
//! behaviour change" guarantee from the T3 spec.

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use mewcode_client::runtime::model::{App, Cmd, Msg, Screen};
use mewcode_client::runtime::update::update;

fn mouse_click(x: u16, y: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::empty(),
    }
}

/// A `Msg::Mouse` on the default `Home` screen returns `Cmd::None`
/// and leaves the screen, toast, and quit flag unchanged.
#[test]
fn home_screen_ignores_mouse() {
    let mut app = App::new();
    let initial_screen = matches!(app.screen, Screen::Home(_));
    let initial_quit = app.should_quit;
    assert!(app.toast.is_none());

    let cmd = update(&mut app, Msg::Mouse(mouse_click(10, 5)));

    assert!(matches!(cmd, Cmd::None));
    assert_eq!(initial_screen, matches!(app.screen, Screen::Home(_)));
    assert_eq!(app.should_quit, initial_quit);
    assert!(app.toast.is_none());
}

/// A `Msg::Mouse` on a `NewSession` screen also returns `Cmd::None`
/// and leaves the screen and toast unchanged — covers the form-screen
/// path where accidental click handling could swallow or mutate input.
#[test]
fn new_session_screen_ignores_mouse() {
    let mut app = App::new();
    app.screen = Screen::NewSession(Default::default());

    let cmd = update(&mut app, Msg::Mouse(mouse_click(0, 0)));

    assert!(matches!(cmd, Cmd::None));
    assert!(matches!(app.screen, Screen::NewSession(_)));
    assert!(app.toast.is_none());
}

/// Sanity check: a click event's *content* (column / row / button)
/// cannot influence the app. The future T5 handlers *will* read
/// column / row, but until then the model must be fully decoupled
/// from mouse events.
#[test]
fn mouse_event_content_does_not_reach_app_state() {
    let mut app = App::new();

    for &(x, y) in &[(0u16, 0u16), (79, 0), (0, 23), (79, 23), (40, 12)] {
        update(&mut app, Msg::Mouse(mouse_click(x, y)));
    }

    assert!(matches!(app.screen, Screen::Home(_)));
    assert!(!app.should_quit);
    assert!(app.toast.is_none());
}
