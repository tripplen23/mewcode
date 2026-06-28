//! Compact one-line "card" for a tool invocation in the chat transcript.
//!
//! Replaces the previous raw `→ name({input})` rendering (which dumped the
//! full JSON inline and used the same Magenta for everything). The card
//! shows the tool name, a one-line summary of the arguments, and a one-line
//! preview of the result. Long values are truncated with `…`.
//!
//! Colors are hardcoded for now. P14.3 will introduce a [`Theme`] so card
//! colors (and every other color in the app) read from a single source.
//!
//! [`Theme`]: crate::runtime::view::theme
//!
//! Ceiling: no expand/collapse toggle, no syntax-highlighted result body,
//! no streaming tool cards (only committed `MessagePart` variants). Those
//! are follow-ups once the basic shape is in.
//!
//! ## Visibility
//!
//! The three `render_*` functions and the two helper functions are all
//! `pub` (not `pub(super)` or private) so that integration tests in
//! `crates/client/tests/tool_card.rs` can drive them through the public
//! surface — see `CONTRIBUTING.md` §"Tests". The two helpers are marked
//! `#[doc(hidden)]` because they are test scaffolding, not part of the
//! view API that downstream consumers (i.e. the rest of the app) should
//! rely on; they may change shape without a major-version bump.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use mewcode_protocol::{ToolCall, ToolResult};

const MAX_ARGS_CHARS: usize = 60;
const MAX_RESULT_LINES: usize = 2;
const MAX_RESULT_LINE_CHARS: usize = 80;

/// Render a `▸` header line for a tool call. The full arguments are
/// inlined as a one-line summary; long values are truncated with `…`.
pub fn render_tool_call_header(call: &ToolCall) -> Line<'static> {
    let args = truncate_one_line(&summarise_json(&call.input), MAX_ARGS_CHARS);
    Line::from(vec![
        Span::styled("▸ ", Style::default().fg(Color::Cyan)),
        Span::styled(
            format!("{}({args})", call.name),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

/// Render the result body lines (indented under the header). Returns an
/// empty `Vec` if the result is `null` or empty.
pub fn render_tool_result_body(res: &ToolResult) -> Vec<Line<'static>> {
    let summary = summarise_json(&res.output);
    if summary.is_empty() {
        return Vec::new();
    }
    let prefix = if res.is_error { "⎿ error: " } else { "⎿ " };
    let color = if res.is_error {
        Color::Red
    } else {
        Color::DarkGray
    };
    let mut out: Vec<Line<'static>> = Vec::new();
    for (i, line) in summary.lines().take(MAX_RESULT_LINES).enumerate() {
        let text = truncate_one_line(line, MAX_RESULT_LINE_CHARS);
        let prefix = if i == 0 { prefix } else { "  " };
        out.push(Line::from(Span::styled(
            format!("{prefix}{text}"),
            Style::default().fg(color),
        )));
    }
    out
}

/// Render a `←` header for a standalone `ToolResult` (used when the result
/// appears without its `ToolCall`, e.g. tool messages injected from the
/// model rather than the assistant).
pub fn render_tool_result_header(res: &ToolResult) -> Line<'static> {
    let color = if res.is_error {
        Color::Red
    } else {
        Color::DarkGray
    };
    Line::from(Span::styled(
        format!(
            "← {} {}",
            res.name,
            if res.is_error { "error" } else { "ok" }
        ),
        Style::default().fg(color),
    ))
}

/// Compact one-line summary of a JSON value: stringify scalars directly,
/// objects show `"{k: v, k2: v2}"`, arrays show `"[n items]"`, `null` is
/// the empty string.
///
/// Test surface only — `#[doc(hidden)]` because the shape is an internal
/// detail of how the card fits on one line and may change without notice.
#[doc(hidden)]
pub fn summarise_json(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(a) => {
            format!("[{} item{}]", a.len(), if a.len() == 1 { "" } else { "s" })
        }
        serde_json::Value::Object(o) => {
            let mut parts: Vec<String> = o
                .iter()
                .map(|(k, v)| {
                    let vs = summarise_json(v);
                    if vs.is_empty() {
                        k.to_string()
                    } else if vs.len() > 24 {
                        format!("{k}: …")
                    } else {
                        format!("{k}: {vs}")
                    }
                })
                .collect();
            parts.sort();
            format!("{{{}}}", parts.join(", "))
        }
    }
}

/// Truncate a string on the first line so the result has at most
/// `max_chars` characters. If the input was cut, the last character is
/// replaced by `…` (a single-character ellipsis), keeping the total
/// width at the configured cap. With `max_chars == 0`, the result is
/// always `…` (one char), since the caller's cap is zero.
///
/// Test surface only — `#[doc(hidden)]` because the truncation policy is
/// a render detail, not a stable API.
#[doc(hidden)]
pub fn truncate_one_line(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return "…".to_string();
    }
    let first = s.lines().next().unwrap_or("");
    let cap_minus_marker = max_chars - 1;
    if first.chars().count() <= max_chars {
        first.to_string()
    } else {
        let cut: String = first.chars().take(cap_minus_marker).collect();
        format!("{cut}…")
    }
}
