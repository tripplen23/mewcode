use crate::net::ModelEntry;
use mewcode_protocol::{Mode, ModelId};
use std::str::FromStr;
use tui_textarea::TextArea;

/// Which field of the new-session form currently has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewSessionField {
    /// The title text input.
    Title,
    /// The model picker.
    Model,
    /// The mode (Build/Plan) toggle.
    Mode,
}

impl NewSessionField {
    /// The next field in the focus cycle: Title → Model → Mode → Title.
    pub fn next(self) -> Self {
        match self {
            NewSessionField::Title => NewSessionField::Model,
            NewSessionField::Model => NewSessionField::Mode,
            NewSessionField::Mode => NewSessionField::Title,
        }
    }
}

/// The model picker's two observable states.
///
/// > Idiom: make illegal states unrepresentable. A single
/// > [`ModelPicker::Loaded`] (carrying the invariant that `models` is
/// > non-empty) replaces an `Option<Vec<…>>` plus a separate index, so
/// > "an index into a list that isn't loaded yet" cannot be written down.
/// > `Loading` ignores selection changes; `Loaded` always has ≥1 entry.
#[derive(Debug)]
pub enum ModelPicker {
    /// The `GET /models` request is still in flight.
    Loading,
    /// Models are loaded; `selected` indexes the non-empty `models`.
    Loaded {
        /// The available models, in display order. Invariant: non-empty.
        models: Vec<ModelId>,
        /// Index of the highlighted model within `models`.
        selected: usize,
    },
}

impl ModelPicker {
    /// Build a loaded picker from a `GET /models` result, returning the picker
    /// and an optional error indication.
    ///
    /// A non-empty result whose ids are known maps one [`ModelId`] per entry in
    /// returned order with index 0 selected and no error. A failure, an empty
    /// list, or an all-unknown list falls back to the built-in [`ModelId::ALL`]
    /// list (index 0 selected) with a "couldn't load models" error.
    pub fn from_registry(result: Result<Vec<ModelEntry>, String>) -> (Self, Option<String>) {
        let models: Vec<ModelId> = match result {
            Ok(entries) => entries
                .iter()
                .filter_map(|e| ModelId::from_str(&e.id).ok())
                .collect(),
            Err(_) => Vec::new(),
        };
        if models.is_empty() {
            (
                ModelPicker::Loaded {
                    models: ModelId::ALL.to_vec(),
                    selected: 0,
                },
                Some("couldn't load models".to_string()),
            )
        } else {
            (
                ModelPicker::Loaded {
                    models,
                    selected: 0,
                },
                None,
            )
        }
    }

    /// The currently selected model, or `None` while still `Loading`.
    pub fn selected_model(&self) -> Option<ModelId> {
        match self {
            ModelPicker::Loading => None,
            ModelPicker::Loaded { models, selected } => models.get(*selected).copied(),
        }
    }

    /// Display name of the selected model, or a loading placeholder.
    pub fn display_name(&self) -> &'static str {
        match self.selected_model() {
            Some(model) => model.display_name(),
            None => "loading…",
        }
    }

    /// Select the previous entry, clamping at the first (no wrap). No-op while `Loading`.
    pub fn select_prev(&mut self) {
        if let ModelPicker::Loaded { selected, .. } = self {
            *selected = selected.saturating_sub(1);
        }
    }

    /// Select the next entry, clamping at the last (no wrap). No-op while `Loading`.
    pub fn select_next(&mut self) {
        if let ModelPicker::Loaded { models, selected } = self {
            *selected = (*selected + 1).min(models.len().saturating_sub(1));
        }
    }
}

/// State backing [`super::Screen::NewSession`].
#[derive(Debug)]
pub struct NewSessionState {
    /// The session title editor.
    pub title: TextArea<'static>,
    /// The model picker, fed from the server registry.
    pub model: ModelPicker,
    /// Selected interaction mode.
    pub mode: Mode,
    /// Which field currently has focus.
    pub field: NewSessionField,
    /// `true` while a `POST /sessions` request is in flight.
    pub submitting: bool,
    /// Persistent dialog error/hint, cleared on the next edit/resubmit.
    pub error: Option<String>,
}

impl Default for NewSessionState {
    fn default() -> Self {
        Self {
            title: TextArea::default(),
            model: ModelPicker::Loading,
            mode: Mode::default(),
            field: NewSessionField::Title,
            submitting: false,
            error: None,
        }
    }
}
