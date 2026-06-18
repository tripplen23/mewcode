//! Pure helpers for the single-turn completion path: pick the user text to
//! send, and fold a rig completion back into one reply string. They are
//! deliberately free of the runtime, the network, and the mpsc channel so the
//! engine's external `tests/*.rs` can property-test the turn's core logic
//! directly — hence they are re-exported `pub` from the harness module rather
//! than buried as private fns the integration tests could not reach.

use mewcode_protocol::{Message, MessagePart, Role};
use rig_core::OneOrMany;
use rig_core::completion::message::AssistantContent;

/// Text of the most recent [`Role::User`] message — every [`MessagePart::Text`]
/// of that message concatenated in order — or `None` when the history holds no
/// user message.
pub fn last_user_text(messages: &[Message]) -> Option<String> {
    messages
        .iter()
        .rev()
        .find(|m| m.role == Role::User)
        .map(|m| {
            m.parts
                .iter()
                .filter_map(|p| match p {
                    MessagePart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect()
        })
}

/// Concatenate every text segment of a rig completion in returned order,
/// dropping non-text segments (tool calls, reasoning, images). A completion
/// with no text segments yields an empty string. The choice is the
/// [`OneOrMany<AssistantContent>`](https://docs.rs/rig-core/latest/rig_core/one_or_many/struct.OneOrMany.html)
/// returned by
/// [`AssistantContent`](https://docs.rs/rig-core/latest/rig_core/completion/message/enum.AssistantContent.html).
pub fn reply_text(choice: &OneOrMany<AssistantContent>) -> String {
    choice
        .iter()
        .filter_map(|c| match c {
            AssistantContent::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect()
}
