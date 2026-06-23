//! Property test for the model-registry → picker mapping.
//!
//! Exercises [`ModelPicker::from_registry`] through its public API: a non-empty
//! `GET /models` result whose ids are all known must produce a `Loaded` picker
//! listing one `ModelId` per entry in the returned order, with index 0 selected
//! and no error.

// Feature: session-flow-and-engine-v0 Registry mapping preserves order with the first selected

use mewcode_client::net::ModelEntry;
use mewcode_client::runtime::model::ModelPicker;
use mewcode_protocol::ModelId;
use proptest::prelude::*;

/// Build a `ModelEntry` whose `id` is the model's known provider id.
fn entry_for(model: ModelId) -> ModelEntry {
    ModelEntry {
        id: model.as_str().to_string(),
        display_name: model.display_name().to_string(),
        kind: model.kind(),
    }
}

/// Strategy: a non-empty list of known models (sampled from `ModelId::ALL`),
/// in arbitrary order with repeats allowed — the input space of "ids are all known".
fn known_models() -> impl Strategy<Value = Vec<ModelId>> {
    proptest::collection::vec(0..ModelId::ALL.len(), 1..=24)
        .prop_map(|idxs| idxs.into_iter().map(|i| ModelId::ALL[i]).collect())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn registry_mapping_preserves_order_with_first_selected(models in known_models()) {
        let entries: Vec<ModelEntry> = models.iter().copied().map(entry_for).collect();

        let (picker, error) = ModelPicker::from_registry(Ok(entries));

        // All ids were known, so there is no fallback and no error indication.
        prop_assert!(error.is_none());

        match picker {
            ModelPicker::Loaded { models: loaded, selected } => {
                // One ModelId per entry, in the returned order, index 0 selected.
                prop_assert_eq!(loaded, models);
                prop_assert_eq!(selected, 0);
            }
            ModelPicker::Loading => prop_assert!(false, "expected Loaded, got Loading"),
        }
    }
}
