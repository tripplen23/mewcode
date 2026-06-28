//! Example tests for the new-session success/failure transitions.
//!
//! These drive the pure `update` through `Msg::SessionCreated` and assert the
//! screen transition and recovery behaviour for each outcome:
//! - `Ok` -> Session screen, empty history
//! - `CreateError::Other` -> stay, retain values, persistent error, `submitting` cleared
//! - `CreateError::EmptyTitle` -> keep Title focus + hint
//! - and the persistent error clears on the next edit.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use uuid::Uuid;

use mewcode_client::net::Session;
use mewcode_client::runtime::model::{
    App, CreateError, ModelPicker, Msg, NewSessionField, NewSessionState, Screen,
};
use mewcode_client::runtime::update;
use mewcode_protocol::{Mode, ModelId};

/// A `NewSession` app mid-submit: `submitting` set, the title "my session"
/// typed in, a multi-entry loaded picker at index 1, and `Plan` mode.
fn submitting_app() -> App {
    let mut app = App::new();
    app.screen = Screen::NewSession(NewSessionState {
        model: ModelPicker::Loaded {
            models: ModelId::ALL.to_vec(),
            selected: 1,
        },
        mode: Mode::Plan,
        field: NewSessionField::Title,
        submitting: true,
        ..NewSessionState::default()
    });
    // Type the title through the field so we don't depend on `TextArea`'s API.
    for c in "my session".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    app
}

fn key(app: &mut App, code: KeyCode) {
    update(app, Msg::Key(KeyEvent::new(code, KeyModifiers::NONE)));
}

fn new_session(app: &App) -> &NewSessionState {
    match &app.screen {
        Screen::NewSession(n) => n,
        other => panic!("expected NewSession, got {other:?}"),
    }
}

fn made_session() -> Session {
    Session {
        id: Uuid::new_v4(),
        title: "my session".to_string(),
        model: ModelId::default(),
        mode: Mode::default(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        messages: vec![],
    }
}

#[test]
fn ok_transitions_to_session_with_empty_history() {
    let mut app = submitting_app();
    let session = made_session();
    let id = session.id;

    update(&mut app, Msg::SessionCreated(Ok(session)));

    match &app.screen {
        Screen::Workspace(ws) => {
            let s = ws.chat.as_ref().expect("session was created");
            assert_eq!(s.session.id, id);
            assert!(s.session.messages.is_empty(), "new session starts empty");
        }
        other => panic!("expected Workspace, got {other:?}"),
    }
}

#[test]
fn other_error_stays_retains_values_and_clears_submitting() {
    let mut app = submitting_app();

    update(
        &mut app,
        Msg::SessionCreated(Err(CreateError::Other("server returned status 500".into()))),
    );

    let n = new_session(&app);
    // Stayed on the dialog, retained title/model/mode.
    assert_eq!(n.title.lines().join("\n"), "my session");
    assert_eq!(n.model.selected_model(), Some(ModelId::ALL[1]));
    assert_eq!(n.mode, Mode::Plan);
    // Persistent error set, in-flight flag cleared.
    assert_eq!(n.error.as_deref(), Some("server returned status 500"));
    assert!(!n.submitting);
}

#[test]
fn empty_title_error_keeps_focus_and_shows_hint() {
    let mut app = submitting_app();

    update(
        &mut app,
        Msg::SessionCreated(Err(CreateError::EmptyTitle(
            "server returned status 400".into(),
        ))),
    );

    let n = new_session(&app);
    assert_eq!(n.field, NewSessionField::Title);
    assert!(n.error.is_some(), "expected the required-title hint");
    assert!(!n.submitting);
    // Values are retained for the user to correct.
    assert_eq!(n.title.lines().join("\n"), "my session");
}

#[test]
fn persistent_error_clears_on_next_edit() {
    let mut app = submitting_app();
    update(
        &mut app,
        Msg::SessionCreated(Err(CreateError::Other("boom".into()))),
    );
    assert!(new_session(&app).error.is_some());

    // Typing a character edits the title and clears the persistent error.
    key(&mut app, KeyCode::Char('x'));
    assert!(new_session(&app).error.is_none());
}
