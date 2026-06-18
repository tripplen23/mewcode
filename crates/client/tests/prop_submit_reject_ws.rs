// Feature: session-flow-and-engine-v0, Property 6: Whitespace-only titles are rejected
//
// For any string composed solely of whitespace (including the empty string),
// pressing `Enter` submits no request (returns `Cmd::None`), keeps focus on the
// Title field, and sets the dialog error/hint.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use proptest::prelude::*;

use mewcode_client::runtime::model::{App, Cmd, Msg, NewSessionField, NewSessionState, Screen};
use mewcode_client::runtime::update;

/// Whitespace characters that the Title field inserts as literal text.
///
/// All are typed as `KeyCode::Char(..)`, so none collides with the `Tab` key
/// (focus cycle) or `Enter` (submit): a literal `'\t'` is `Char('\t')`, not
/// `KeyCode::Tab`. `str::trim` treats every one as whitespace.
fn ws_chars() -> impl Strategy<Value = Vec<char>> {
    prop::collection::vec(prop_oneof![Just(' '), Just('\t'), Just('\u{a0}')], 0..20)
}

fn new_session_app() -> App {
    let mut app = App::new();
    app.screen = Screen::NewSession(NewSessionState::default());
    app
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn whitespace_only_titles_are_rejected(ws in ws_chars()) {
        let mut app = new_session_app();

        for c in &ws {
            update(&mut app, Msg::Key(KeyEvent::new(KeyCode::Char(*c), KeyModifiers::NONE)));
        }

        let cmd = update(&mut app, Msg::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)));
        prop_assert!(matches!(cmd, Cmd::None), "expected Cmd::None, got {:?}", cmd);

        let Screen::NewSession(n) = &app.screen else {
            panic!("expected to remain on NewSession");
        };
        prop_assert_eq!(n.field, NewSessionField::Title);
        prop_assert!(n.error.is_some(), "expected the required-title hint to be set");
        prop_assert!(!n.submitting, "rejected submit must not mark a request in flight");
    }
}
