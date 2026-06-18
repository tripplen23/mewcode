// Feature: session-flow-and-engine-v0, submit is ignored while a request is in flight
//
// For any dialog state with `submitting == true`, pressing `Enter` returns
// `Cmd::None` and starts no additional `POST /sessions` — even when the title
// is valid (so it is the in-flight guard, not title validation, that rejects).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use proptest::prelude::*;

use mewcode_client::runtime::model::{
    App, Cmd, ModelPicker, Msg, NewSessionField, NewSessionState, Screen,
};
use mewcode_client::runtime::update;
use mewcode_protocol::{Mode, ModelId};

/// A valid (non-whitespace) title, typed character-by-character.
fn any_title() -> impl Strategy<Value = Vec<char>> {
    let alnum = || (b'a'..=b'z').prop_map(|b| b as char);
    (alnum(), prop::collection::vec(alnum(), 0..8)).prop_map(|(mid, rest)| {
        let mut t = vec![mid];
        t.extend(rest);
        t
    })
}

fn arb_mode() -> impl Strategy<Value = Mode> {
    prop_oneof![Just(Mode::Build), Just(Mode::Plan)]
}

/// A `NewSession` app already marked `submitting`, focused on Title, with a
/// loaded picker.
fn inflight_app(selected: usize, mode: Mode) -> App {
    let mut app = App::new();
    app.screen = Screen::NewSession(NewSessionState {
        model: ModelPicker::Loaded {
            models: ModelId::ALL.to_vec(),
            selected,
        },
        mode,
        field: NewSessionField::Title,
        submitting: true,
        ..NewSessionState::default()
    });
    app
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn submit_ignored_while_in_flight(
        title in any_title(),
        selected in 0usize..ModelId::ALL.len(),
        mode in arb_mode(),
        tabs in 0usize..6,
    ) {
        let mut app = inflight_app(selected, mode);

        // Populate a valid title (typing has no in-flight guard).
        for c in &title {
            update(&mut app, Msg::Key(KeyEvent::new(KeyCode::Char(*c), KeyModifiers::NONE)));
        }
        // Vary the focused field; none of these change the in-flight state.
        for _ in 0..tabs {
            update(&mut app, Msg::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)));
        }

        let cmd = update(&mut app, Msg::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)));

        // No second request, and the dialog stays put still marked in flight.
        prop_assert!(matches!(cmd, Cmd::None), "expected Cmd::None, got {:?}", cmd);
        let Screen::NewSession(n) = &app.screen else {
            panic!("expected to remain on NewSession");
        };
        prop_assert!(n.submitting, "the in-flight flag must be untouched");
    }
}
