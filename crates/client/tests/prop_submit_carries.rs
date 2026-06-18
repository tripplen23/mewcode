// Feature: session-flow-and-engine-v0, submit trims the title and carries the current selection
//
// For any title containing at least one non-whitespace character, pressing
// `Enter` produces a `Cmd::CreateSession` whose title equals the input trimmed
// of leading and trailing whitespace, and whose model and mode equal the
// picker's current selections.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use proptest::prelude::*;

use mewcode_client::runtime::model::{
    App, Cmd, ModelPicker, Msg, NewSessionField, NewSessionState, Screen,
};
use mewcode_client::runtime::update;
use mewcode_protocol::{Mode, ModelId};

/// A single alphanumeric character (mapped from a `u8` range, since proptest
/// has no `Strategy` for `RangeInclusive<char>`).
fn alnum() -> impl Strategy<Value = char> {
    prop_oneof![
        (b'a'..=b'z').prop_map(|b| b as char),
        (b'0'..=b'9').prop_map(|b| b as char),
    ]
}

/// Optional alphanumeric/space run, used for the leading and trailing parts.
fn segment() -> impl Strategy<Value = Vec<char>> {
    prop::collection::vec(
        prop_oneof![Just(' '), (b'a'..=b'z').prop_map(|b| b as char)],
        0..8,
    )
}

/// A title that always has at least one non-whitespace character: optional
/// leading/trailing spaces around a guaranteed-non-space alphanumeric core.
/// Typed character-by-character, so it never contains Tab/Enter/newline.
fn any_title() -> impl Strategy<Value = Vec<char>> {
    (segment(), alnum(), segment()).prop_map(|(mut pre, mid, suf)| {
        pre.push(mid);
        pre.extend(suf);
        pre
    })
}

fn arb_mode() -> impl Strategy<Value = Mode> {
    prop_oneof![Just(Mode::Build), Just(Mode::Plan)]
}

/// A `NewSession` app focused on Title, with a loaded multi-entry picker at
/// `selected` and the given `mode`, and an empty title.
fn submit_app(selected: usize, mode: Mode) -> App {
    let mut app = App::new();
    app.screen = Screen::NewSession(NewSessionState {
        model: ModelPicker::Loaded {
            models: ModelId::ALL.to_vec(),
            selected,
        },
        mode,
        field: NewSessionField::Title,
        ..NewSessionState::default()
    });
    app
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn submit_trims_title_and_carries_selection(
        title in any_title(),
        selected in 0usize..ModelId::ALL.len(),
        mode in arb_mode(),
    ) {
        let mut app = submit_app(selected, mode);

        // Type the title; this never disturbs the pickers.
        for c in &title {
            update(&mut app, Msg::Key(KeyEvent::new(KeyCode::Char(*c), KeyModifiers::NONE)));
        }

        let expected_title: String = title.iter().collect::<String>().trim().to_string();
        let expected_model = ModelId::ALL[selected];

        match update(&mut app, Msg::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))) {
            Cmd::CreateSession(req) => {
                prop_assert_eq!(req.title, expected_title);
                prop_assert_eq!(req.model, Some(expected_model));
                prop_assert_eq!(req.mode, Some(mode));
            }
            other => prop_assert!(false, "expected CreateSession, got {:?}", other),
        }
    }
}
