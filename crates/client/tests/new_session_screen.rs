//! New-session dialog snapshot tests.
//!
//! Renders the `NewSession` screen in its key states into a ratatui
//! `TestBackend` and pins each buffer with `insta`. This locks the model
//! loading indication, the focused-field highlight, the persistent
//! error/hint line, and the in-progress ("Creating session…") indication
//! against accidental regressions.

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use mewcode_client::runtime::model::{App, ModelPicker, NewSessionField, NewSessionState, Screen};
use mewcode_client::runtime::view::render;
use mewcode_protocol::{Mode, ModelId};

/// Render the given `NewSession` state and return the backend's text buffer.
fn render_new_session(state: NewSessionState) -> String {
    let mut app = App::new();
    app.screen = Screen::NewSession(state);
    let mut terminal = Terminal::new(TestBackend::new(60, 14)).unwrap();
    terminal.draw(|frame| render(frame, &mut app)).unwrap();
    terminal.backend().to_string()
}

#[test]
fn loading_models() {
    // Picker still loading: the Model field shows a loading indication.
    insta::assert_snapshot!(render_new_session(NewSessionState::default()));
}

#[test]
fn loaded_model_focused() {
    // Models loaded; focus on the Model field (brightened border).
    insta::assert_snapshot!(render_new_session(NewSessionState {
        model: ModelPicker::Loaded {
            models: ModelId::ALL.to_vec(),
            selected: 0,
        },
        field: NewSessionField::Model,
        mode: Mode::Build,
        ..NewSessionState::default()
    }));
}

#[test]
fn error_line() {
    // Fallback picker plus the persistent "couldn't load models" error.
    insta::assert_snapshot!(render_new_session(NewSessionState {
        model: ModelPicker::Loaded {
            models: ModelId::ALL.to_vec(),
            selected: 0,
        },
        field: NewSessionField::Title,
        error: Some("couldn't load models".to_string()),
        ..NewSessionState::default()
    }));
}

#[test]
fn submitting() {
    // A create is in flight: the in-progress indication is shown.
    insta::assert_snapshot!(render_new_session(NewSessionState {
        model: ModelPicker::Loaded {
            models: ModelId::ALL.to_vec(),
            selected: 0,
        },
        field: NewSessionField::Title,
        submitting: true,
        ..NewSessionState::default()
    }));
}
