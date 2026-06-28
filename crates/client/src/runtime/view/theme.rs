//! Semantic theme slots for the TUI — Warp-style named slots so the whole
//! app reads colors from one place, never hardcoded.
//!
//! See `docs/architecture-canvas/ui-aesthetic.md` §4 for the design. We
//! ship one good default; theme switching UI + multiple shipped themes are
//! M2 per §6.
//!
//! `ponytail:` this module is the single source of truth for color. View
//! code never instantiates `Color::Xxx` directly — it asks the theme. The
//! theme itself never renders anything.
//!
//! Ceiling: M1 is a fixed single theme. M2 will add a TOML loader +
//! runtime switcher; that lives behind the same `Theme` API so the
//! call-sites don't change.

use ratatui::style::{Color, Style};

/// Semantic color slots used across the workspace. Slots are resolved to
/// concrete `Color`s by [`Theme::resolve`].
///
/// Order follows `ui-aesthetic.md` §4 so the slot names match the doc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// Main background.
    Bg,
    /// Elevated surface (pane interiors, block strip, etc.).
    Surface,
    /// Dotted grid dot on the canvas.
    GridDot,
    /// Default foreground text.
    Fg,
    /// Dimmed foreground (status bars, hints).
    FgDim,
    /// Primary accent (selection highlight, prompt glyph).
    Accent,
    /// Secondary accent (hover/secondary states).
    AccentAlt,
    /// C4 *System* node title bar.
    NodeSystem,
    /// C4 *Container* node title bar.
    NodeContainer,
    /// C4 *Component* node title bar.
    NodeComponent,
    /// Default edge line.
    Edge,
    /// Edge incident to the selected node.
    EdgeSelected,
    /// Arrowhead glyph at the edge target.
    Arrowhead,
    /// Success badge (tool call OK, ✓).
    Success,
    /// Warning badge.
    Warning,
    /// Error badge (toast, ✗).
    Error,
    /// Block strip left accent bar (`▍`).
    BlockBar,
}

/// The M1 default theme — a single fixed palette. Resolved at startup;
/// modules grab it via [`Theme::default`] and never rebuild.
///
/// We keep this as a free function (not a method) so the call-site is
/// `let s = style(Slot::Edge);` — no `theme.style(...)` boilerplate.
pub fn style(slot: Slot) -> Style {
    let color = color_for(slot);
    Style::default().fg(color)
}

/// Background color for a slot, when a widget needs to fill its area.
/// Not every slot has a background (e.g. `GridDot` is just a glyph).
pub fn bg(slot: Slot) -> Color {
    color_for(slot)
}

fn color_for(slot: Slot) -> Color {
    match slot {
        Slot::Bg => Color::Reset,
        Slot::Surface => Color::Reset,
        Slot::GridDot => Color::DarkGray,
        Slot::Fg => Color::Reset,
        Slot::FgDim => Color::DarkGray,
        Slot::Accent => Color::Cyan,
        Slot::AccentAlt => Color::Magenta,
        // C4 taxonomy: blue/cyan/green matches draw.io convention so
        // C4-familiar users see the same colors as in their docs.
        Slot::NodeSystem => Color::Blue,
        Slot::NodeContainer => Color::Cyan,
        Slot::NodeComponent => Color::Green,
        Slot::Edge => Color::DarkGray,
        Slot::EdgeSelected => Color::Cyan,
        Slot::Arrowhead => Color::DarkGray,
        Slot::Success => Color::Green,
        Slot::Warning => Color::Yellow,
        Slot::Error => Color::Red,
        Slot::BlockBar => Color::Cyan,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Theme is deterministic: the same slot always returns the same
    /// color. If M2 ever adds theme switching, this test must still
    /// pass for the *default* theme.
    #[test]
    fn slot_colors_are_stable() {
        assert_eq!(color_for(Slot::Accent), Color::Cyan);
        assert_eq!(color_for(Slot::NodeSystem), Color::Blue);
        assert_eq!(color_for(Slot::NodeContainer), Color::Cyan);
        assert_eq!(color_for(Slot::NodeComponent), Color::Green);
        assert_eq!(color_for(Slot::Error), Color::Red);
    }

    /// Every slot resolves without panicking — guards against the
    /// "added a new variant, forgot to map it" footgun.
    #[test]
    fn all_slots_resolve() {
        for slot in [
            Slot::Bg,
            Slot::Surface,
            Slot::GridDot,
            Slot::Fg,
            Slot::FgDim,
            Slot::Accent,
            Slot::AccentAlt,
            Slot::NodeSystem,
            Slot::NodeContainer,
            Slot::NodeComponent,
            Slot::Edge,
            Slot::EdgeSelected,
            Slot::Arrowhead,
            Slot::Success,
            Slot::Warning,
            Slot::Error,
            Slot::BlockBar,
        ] {
            // Just ensure the call returns; the assertion on the
            // result would be tautological (every Color impls Debug).
            let _ = style(slot);
        }
    }
}
