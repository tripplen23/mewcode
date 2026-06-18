// Feature: session-flow-and-engine-v0, any registry failure falls
// back to the built-in list.
//
// For any failed `GET /models` result, an empty registry, or a registry whose
// ids are all unknown, `ModelPicker::from_registry` yields a `Loaded` picker
// carrying the built-in `ModelId::ALL` list in its defined order with index 0
// selected, and an error indication set.

use std::str::FromStr;

use proptest::prelude::*;

use mewcode_client::net::ModelEntry;
use mewcode_client::runtime::model::ModelPicker;
use mewcode_protocol::{ModelId, ModelKind};

/// A string that does not parse to any known [`ModelId`] (matches neither a
/// provider id nor a display name).
fn unknown_id() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 ._-]{0,12}".prop_filter("must not parse to a known ModelId", |s| {
        ModelId::from_str(s).is_err()
    })
}

/// A registry entry whose id is guaranteed unknown to the protocol.
fn unknown_entry() -> impl Strategy<Value = ModelEntry> {
    (unknown_id(), any::<bool>()).prop_map(|(id, anthropic)| ModelEntry {
        id,
        display_name: "ignored".to_string(),
        kind: if anthropic {
            ModelKind::AnthropicMessages
        } else {
            ModelKind::OpenAiChatCompletions
        },
    })
}

/// The three failure modes collapses into a fallback: a failed
/// result, an empty registry, or a non-empty registry of all-unknown ids.
fn failing_registry() -> impl Strategy<Value = Result<Vec<ModelEntry>, String>> {
    prop_oneof![
        ".*".prop_map(Err),
        Just(Ok(Vec::new())),
        prop::collection::vec(unknown_entry(), 1..6).prop_map(Ok),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn registry_failure_falls_back_to_builtin_list(result in failing_registry()) {
        let (picker, error) = ModelPicker::from_registry(result);

        match picker {
            ModelPicker::Loaded { models, selected } => {
                // Built-in list, in its defined display order.
                prop_assert_eq!(models, ModelId::ALL.to_vec());
                // First entry selected.
                prop_assert_eq!(selected, 0);
            }
            ModelPicker::Loading => {
                prop_assert!(false, "expected Loaded fallback, got Loading");
            }
        }

        // The dialog error indication is set on any fallback.
        prop_assert!(error.is_some());
    }
}
