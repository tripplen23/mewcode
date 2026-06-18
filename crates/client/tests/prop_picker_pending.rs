// Feature: session-flow-and-engine-v0, Property 10: A pending picker ignores selection changes
//
// For any sequence of Left/Right presses while the picker is `Loading`, the
// picker remains `Loading` and exposes no changed selection (its selected model
// stays `None`) until the load succeeds or fails.

use mewcode_client::runtime::model::ModelPicker;
use proptest::prelude::*;

/// `false` => Left/select_prev, `true` => Right/select_next.
fn any_presses() -> impl Strategy<Value = Vec<bool>> {
    prop::collection::vec(any::<bool>(), 0..50)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn loading_picker_ignores_selection_changes(presses in any_presses()) {
        let mut picker = ModelPicker::Loading;

        for right in presses {
            if right {
                picker.select_next();
            } else {
                picker.select_prev();
            }
            // Stays Loading, exposing no selection, after every press.
            prop_assert!(
                matches!(picker, ModelPicker::Loading),
                "picker left the Loading state"
            );
            prop_assert!(
                picker.selected_model().is_none(),
                "Loading picker exposed a selected model"
            );
        }
    }
}
