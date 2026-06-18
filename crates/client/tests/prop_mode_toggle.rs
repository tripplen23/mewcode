// Feature: session-flow-and-engine-v0, Property 3: Mode toggle is an involution
//
// For any `Mode`, one `Left`/`Right` press while the Mode field is focused
// yields the other mode, and two presses return the original mode.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use proptest::prelude::*;

use mewcode_client::runtime::model::{App, NewSessionField, NewSessionState, Screen};
use mewcode_client::runtime::update;
use mewcode_protocol::Mode;

/// Build a NewSession app focused on the Mode field with the given mode.
fn mode_app(mode: Mode) -> App {
    let mut app = App::new();
    let state = NewSessionState {
        mode,
        field: NewSessionField::Mode,
        ..NewSessionState::default()
    };
    app.screen = Screen::NewSession(state);
    app
}

fn press(app: &mut App, code: KeyCode) {
    update(
        app,
        mewcode_client::runtime::model::Msg::Key(KeyEvent::new(code, KeyModifiers::NONE)),
    );
}

fn current_mode(app: &App) -> Mode {
    match &app.screen {
        Screen::NewSession(n) => n.mode,
        other => panic!("expected NewSession, got {other:?}"),
    }
}

fn arb_mode() -> impl Strategy<Value = Mode> {
    any::<bool>().prop_map(|b| if b { Mode::Build } else { Mode::Plan })
}

fn arb_dir() -> impl Strategy<Value = KeyCode> {
    any::<bool>().prop_map(|b| if b { KeyCode::Left } else { KeyCode::Right })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// One Left/Right press toggles to the other mode (the two-element involution).
    #[test]
    fn one_press_yields_other_mode(start in arb_mode(), dir in arb_dir()) {
        let mut app = mode_app(start);
        press(&mut app, dir);
        prop_assert_ne!(current_mode(&app), start);
    }

    /// Two presses (in any direction combination) return to the original mode.
    #[test]
    fn two_presses_return_original(start in arb_mode(), d1 in arb_dir(), d2 in arb_dir()) {
        let mut app = mode_app(start);
        press(&mut app, d1);
        press(&mut app, d2);
        prop_assert_eq!(current_mode(&app), start);
    }
}
