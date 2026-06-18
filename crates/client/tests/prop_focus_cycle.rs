//! Property test for the new-session focus cycle.
//!
//! Each `Tab` press advances the focused field via
//! [`NewSessionField::next`]. This proves the cycle is a *closed 3-cycle*:
//! starting from any field, advancing `n` times lands on the same field as
//! advancing `n mod 3` times, and the three fields are all distinct (so the
//! period is exactly 3, not a divisor of it).

// Feature: session-flow-and-engine-v0, Property 1: Focus cycle is a closed 3-cycle

use mewcode_client::runtime::model::NewSessionField;
use proptest::prelude::*;

/// Strategy: any one of the three focusable fields.
fn any_field() -> impl Strategy<Value = NewSessionField> {
    prop_oneof![
        Just(NewSessionField::Title),
        Just(NewSessionField::Model),
        Just(NewSessionField::Mode),
    ]
}

/// Advance the focus `n` times, as `n` consecutive `Tab` presses would.
fn advance(mut field: NewSessionField, n: usize) -> NewSessionField {
    for _ in 0..n {
        field = field.next();
    }
    field
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn focus_cycle_is_a_closed_3_cycle(start in any_field(), n in 0usize..1000) {
        // Advancing `n` times equals advancing `n mod 3` times.
        prop_assert_eq!(advance(start, n), advance(start, n % 3));
    }

    /// The three fields are distinct, so the period is exactly 3 — a Tab press
    /// always changes the field, and three presses return to the start.
    #[test]
    fn three_presses_return_to_start(start in any_field()) {
        prop_assert_ne!(start, start.next());
        prop_assert_ne!(start, advance(start, 2));
        prop_assert_eq!(start, advance(start, 3));
    }
}
