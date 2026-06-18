//! Property-based test for the new-session dialog's title isolation.
//!
//! Feature: session-flow-and-engine-v0, Property 4: Title editing never
//! disturbs the pickers — for any sequence of printable, Backspace, Delete,
//! Left, or Right keys while the Title field is focused, the Model picker's
//! selection and the selected Mode are unchanged.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use proptest::prelude::*;

use mewcode_client::runtime::model::{
    App, ModelPicker, Msg, NewSessionField, NewSessionState, Screen,
};
use mewcode_client::runtime::update;
use mewcode_protocol::{Mode, ModelId};

/// Keys the Title field accepts: printable chars, Backspace, Delete, Left,
/// Right. These are exactly the keys the property ranges over.
fn title_key_strategy() -> impl Strategy<Value = KeyCode> {
    prop_oneof![
        // Printable ASCII, the bulk of real typing.
        (0x20u8..=0x7eu8).prop_map(|b| KeyCode::Char(b as char)),
        Just(KeyCode::Backspace),
        Just(KeyCode::Delete),
        Just(KeyCode::Left),
        Just(KeyCode::Right),
    ]
}

/// A `NewSession` app focused on the Title field with a multi-entry, loaded
/// picker at `selected` and the given `mode`.
fn title_focused_app(selected: usize, mode: Mode) -> App {
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
    fn title_editing_never_disturbs_pickers(
        selected in 0usize..ModelId::ALL.len(),
        mode in prop_oneof![Just(Mode::Build), Just(Mode::Plan)],
        keys in proptest::collection::vec(title_key_strategy(), 0..40),
    ) {
        let mut app = title_focused_app(selected, mode);

        let expected_model = ModelId::ALL[selected];
        for code in keys {
            update(&mut app, Msg::Key(KeyEvent::new(code, KeyModifiers::NONE)));
        }

        let Screen::NewSession(n) = &app.screen else {
            panic!("expected to remain on NewSession");
        };
        // Focus never leaves Title, so neither picker may move.
        prop_assert_eq!(n.field, NewSessionField::Title);
        prop_assert_eq!(n.model.selected_model(), Some(expected_model));
        prop_assert_eq!(n.mode, mode);
    }
}
