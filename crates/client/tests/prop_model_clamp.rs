// Feature: session-flow-and-engine-v0, Property 2: Model picker clamps without wrapping
//
// For any loaded model list and any sequence of Left/Right presses, the
// selection stays within `[0, len-1]`, `Left` (select_prev) never increases it,
// `Right` (select_next) never decreases it, and a single-entry list never moves.

use mewcode_client::runtime::model::ModelPicker;
use mewcode_protocol::ModelId;
use proptest::prelude::*;

/// A non-empty list of models, drawn (with repetition) from `ModelId::ALL`.
fn any_models() -> impl Strategy<Value = Vec<ModelId>> {
    prop::collection::vec(prop::sample::select(ModelId::ALL.to_vec()), 1..=14)
}

/// `false` => Left/select_prev, `true` => Right/select_next.
fn any_presses() -> impl Strategy<Value = Vec<bool>> {
    prop::collection::vec(any::<bool>(), 0..50)
}

fn selected(picker: &ModelPicker) -> usize {
    match picker {
        ModelPicker::Loaded { selected, .. } => *selected,
        ModelPicker::Loading => unreachable!("test only builds Loaded pickers"),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn picker_clamps_without_wrapping(models in any_models(), presses in any_presses()) {
        let len = models.len();
        let single = len == 1;
        let mut picker = ModelPicker::Loaded { models, selected: 0 };

        for right in presses {
            let before = selected(&picker);
            if right {
                picker.select_next();
            } else {
                picker.select_prev();
            }
            let after = selected(&picker);

            // Always in bounds, never wraps.
            prop_assert!(after < len, "selection {after} out of bounds for len {len}");
            if right {
                prop_assert!(after >= before, "Right decreased selection {before} -> {after}");
            } else {
                prop_assert!(after <= before, "Left increased selection {before} -> {after}");
            }
            if single {
                prop_assert_eq!(after, 0, "single-entry list moved to {}", after);
            }
        }
    }
}
