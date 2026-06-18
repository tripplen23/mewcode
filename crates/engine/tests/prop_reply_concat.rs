// Feature: session-flow-and-engine-v0, the reply is exactly the
// concatenation of the completion's text segments
//
// For any completion, `reply_text` equals its text segments concatenated in
// returned order — nothing added (no synthetic placeholder), dropped, or
// reordered. A completion carrying no text (only tool calls / reasoning /
// images, or empty text) yields an empty reply.

use mewcode_engine::harness::reply_text;
use proptest::prelude::*;
use rig_core::OneOrMany;
use rig_core::completion::message::AssistantContent;

/// One generated completion segment. Text segments contribute to the reply;
/// every other variant must be dropped by `reply_text`.
#[derive(Debug, Clone)]
enum Seg {
    Text(String),
    Reasoning(String),
    ToolCall(String),
    Image(String),
}

impl Seg {
    /// The slice this segment contributes to the expected reply: its text for
    /// a `Text` segment, nothing for any other variant.
    fn expected(&self) -> &str {
        match self {
            Seg::Text(t) => t.as_str(),
            _ => "",
        }
    }

    fn into_content(self) -> AssistantContent {
        match self {
            Seg::Text(t) => AssistantContent::text(t),
            Seg::Reasoning(r) => AssistantContent::reasoning(r),
            Seg::ToolCall(name) => {
                AssistantContent::tool_call("call-1", name, serde_json::json!({}))
            }
            Seg::Image(data) => AssistantContent::image_base64(data, None, None),
        }
    }
}

/// Strategy over the four segment kinds, mixing text and non-text so a
/// completion can interleave them in any order.
fn any_seg() -> impl Strategy<Value = Seg> {
    prop_oneof![
        any::<String>().prop_map(Seg::Text),
        any::<String>().prop_map(Seg::Reasoning),
        "[a-z_]{1,16}".prop_map(Seg::ToolCall),
        any::<String>().prop_map(Seg::Image),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn reply_is_concatenation_of_text_segments(segs in prop::collection::vec(any_seg(), 1..8)) {
        // `OneOrMany` is non-empty, so the completion always has ≥1 segment;
        // the "empty reply" case is exercised when those segments carry no
        // text (only non-text variants, or empty `Text` strings).
        let expected: String = segs.iter().map(Seg::expected).collect();

        let contents: Vec<AssistantContent> =
            segs.into_iter().map(Seg::into_content).collect();
        let choice = OneOrMany::many(contents).expect("≥1 segment generated");

        prop_assert_eq!(reply_text(&choice), expected);
    }
}
