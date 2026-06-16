//! Per-screen rendering for the TUI.
//!
//! `view` is a pure function of the model: [`render`] draws the current
//! [`App`] into a ratatui [`Frame`] and never mutates state. Animations
//! (spinner, toast fade) are derived from each `started_at` instant, so a
//! redraw on every 50 ms tick advances them with no model bookkeeping.
//!
//! > Note on dependency versions: `tui-textarea` 0.7 still renders against
//! > ratatui 0.29, but the client draws with ratatui 0.30. Rather than bridge
//! > the two `Widget` traits, the editors are rendered by reading their
//! > `.lines()` and drawing a plain [`Paragraph`] in ratatui 0.30.

use std::sync::OnceLock;
use std::time::Duration;

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use mewcode_protocol::tool::ToolName;
use mewcode_protocol::{MessagePart, Role};

use unicode_width::UnicodeWidthStr;

use super::app::{
    App, HomeState, NewSessionField, NewSessionState, Overlay, Screen, SessionState, Toast,
    ToastKind,
};

/// Braille spinner frames, advanced one step roughly every 80 ms.
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// How long a toast stays fully visible before it begins to fade.
const TOAST_HOLD: Duration = Duration::from_millis(3000);
/// Fade-out duration after the hold window.
const TOAST_FADE: Duration = Duration::from_millis(1000);

/// Draw the whole application: the active screen, then any toast on top.
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    match &app.screen {
        Screen::Home(h) => render_home(frame, area, h),
        Screen::NewSession(n) => render_new_session(frame, area, n),
        Screen::Session(s) => render_session(frame, area, s),
    }

    if let Some(toast) = &app.toast {
        render_toast(frame, area, toast);
    }
}

/// Home: a selectable session list with a loading / empty affordance.
fn render_home(frame: &mut Frame, area: Rect, h: &HomeState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let block = Block::bordered().title(" mewcode — sessions ");

    if h.loading {
        let p = Paragraph::new("Loading sessions…")
            .block(block)
            .alignment(Alignment::Center);
        frame.render_widget(p, chunks[0]);
    } else if h.sessions.is_empty() {
        let p = Paragraph::new(Text::from(vec![
            Line::from(""),
            Line::from("No sessions yet.").alignment(Alignment::Center),
            Line::from("Press 'n' to start a new one.").alignment(Alignment::Center),
        ]))
        .block(block);
        frame.render_widget(p, chunks[0]);
    } else {
        let items: Vec<ListItem> = h
            .sessions
            .iter()
            .map(|s| ListItem::new(s.title.clone()))
            .collect();
        let list = List::new(items)
            .block(block)
            .highlight_symbol("› ")
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        let mut state = ListState::default().with_selected(Some(h.selected));
        frame.render_stateful_widget(list, chunks[0], &mut state);
    }

    let footer = Paragraph::new("n new  •  ↑/↓ select  •  Enter open  •  q quit")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[1]);
}

/// NewSession: title input plus model and mode pickers, with the focused field highlighted.
fn render_new_session(frame: &mut Frame, area: Rect, n: &NewSessionState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title
            Constraint::Length(3), // model
            Constraint::Length(3), // mode
            Constraint::Min(0),    // filler
            Constraint::Length(1), // footer
        ])
        .split(area);

    let title_text = n.title.lines().join("\n");
    frame.render_widget(
        field_block("Title", &title_text, n.field == NewSessionField::Title),
        chunks[0],
    );

    let model = mewcode_protocol::ModelId::ALL
        .get(n.model_idx)
        .copied()
        .unwrap_or_default();
    frame.render_widget(
        field_block(
            "Model  (←/→)",
            &format!("‹ {} ›", model.display_name()),
            n.field == NewSessionField::Model,
        ),
        chunks[1],
    );

    frame.render_widget(
        field_block(
            "Mode  (←/→)",
            &format!("‹ {:?} ›", n.mode),
            n.field == NewSessionField::Mode,
        ),
        chunks[2],
    );

    let footer = Paragraph::new("Tab next field  •  Enter create  •  Esc cancel")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[4]);
}

/// A single labelled field; the border brightens when focused.
fn field_block<'a>(label: &'a str, value: &str, focused: bool) -> Paragraph<'a> {
    let border = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    Paragraph::new(value.to_string()).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border)
            .title(label),
    )
}

/// Session: scrollable transcript, input bar, status bar, plus overlays.
fn render_session(frame: &mut Frame, area: Rect, s: &SessionState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // transcript
            Constraint::Length(3), // input bar
            Constraint::Length(1), // status bar
        ])
        .split(area);

    // --- transcript ---
    let mut lines: Vec<Line> = Vec::new();
    for msg in &s.session.messages {
        lines.extend(render_message(msg));
        lines.push(Line::from(""));
    }
    // The in-flight assistant turn, if any.
    if let Some(st) = &s.streaming {
        lines.push(Line::from(Span::styled(
            format!("{} assistant", spinner_frame(st.started_at.elapsed())),
            Style::default().fg(Color::Yellow),
        )));
        if !st.buffer.is_empty() {
            lines.extend(render_markdown(&st.buffer));
        }
    }

    let transcript = Paragraph::new(Text::from(lines))
        .block(Block::bordered().title(format!(" {} ", s.session.title)))
        .wrap(Wrap { trim: false })
        .scroll((s.scroll, 0));
    frame.render_widget(transcript, chunks[0]);

    // --- input bar ---
    let input_text = s.input.lines().join("\n");
    let input = Paragraph::new(input_text)
        .block(Block::bordered().title(" message "))
        .wrap(Wrap { trim: false });
    frame.render_widget(input, chunks[1]);

    // --- status bar ---
    let status = if s.streaming.is_some() {
        format!(
            "{}  {:?}  •  streaming…",
            s.session.model.display_name(),
            s.session.mode
        )
    } else {
        format!(
            "{}  {:?}  •  /tools  /skills  •  Esc back",
            s.session.model.display_name(),
            s.session.mode
        )
    };
    frame.render_widget(
        Paragraph::new(status).style(Style::default().fg(Color::DarkGray)),
        chunks[2],
    );

    // --- overlays ---
    match s.overlay {
        Overlay::None => {}
        Overlay::Tools => render_overlay(frame, area, "Tools", tools_lines()),
        Overlay::Skills => render_overlay(frame, area, "Skills", skills_lines()),
    }
}

/// Render one persisted message, preserving the arrival order of its parts.
/// Text is markdown; tool calls and results are summarised inline.
fn render_message(msg: &mewcode_protocol::Message) -> Vec<Line<'static>> {
    let (label, label_style) = match msg.role {
        Role::User => ("you", Style::default().fg(Color::Green)),
        Role::Assistant => ("assistant", Style::default().fg(Color::Cyan)),
        Role::Tool => ("tool", Style::default().fg(Color::Magenta)),
    };
    let mut out = vec![Line::from(Span::styled(
        label.to_string(),
        label_style.add_modifier(Modifier::BOLD),
    ))];

    for part in &msg.parts {
        match part {
            MessagePart::Text { text } => out.extend(render_markdown(text)),
            MessagePart::ToolCall(call) => out.push(Line::from(Span::styled(
                format!("→ {}({})", call.name, call.input),
                Style::default().fg(Color::Magenta),
            ))),
            MessagePart::ToolResult(res) => out.push(Line::from(Span::styled(
                format!("← {} result", res.name),
                Style::default().fg(Color::Magenta),
            ))),
            MessagePart::FileMention { path } => out.push(Line::from(Span::styled(
                format!("@{path}"),
                Style::default().fg(Color::Blue),
            ))),
        }
    }
    out
}

/// Render assistant markdown: prose goes through `tui-markdown`, fenced code
/// blocks are highlighted with `syntect`. The fence scanner splits the text
/// so each ```` ``` ```` block is highlighted on its own.
fn render_markdown(text: &str) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut prose = String::new();
    let mut code = String::new();
    let mut lang: Option<String> = None;
    let mut in_code = false;

    let flush_prose = |prose: &mut String, out: &mut Vec<Line<'static>>| {
        if !prose.is_empty() {
            out.extend(own_lines(tui_markdown::from_str(prose)));
            prose.clear();
        }
    };

    for line in text.lines() {
        if let Some(rest) = line.trim_start().strip_prefix("```") {
            if in_code {
                out.extend(highlight_code_block(&code, lang.as_deref()));
                code.clear();
                lang = None;
                in_code = false;
            } else {
                flush_prose(&mut prose, &mut out);
                let tag = rest.trim();
                lang = (!tag.is_empty()).then(|| tag.to_string());
                in_code = true;
            }
        } else if in_code {
            code.push_str(line);
            code.push('\n');
        } else {
            prose.push_str(line);
            prose.push('\n');
        }
    }

    if in_code {
        // Unterminated fence: still render the code we collected.
        out.extend(highlight_code_block(&code, lang.as_deref()));
    } else {
        flush_prose(&mut prose, &mut out);
    }
    out
}

/// Highlight a fenced code block.
///
/// When `lang` is absent or unrecognised, the block is rendered as plain
/// monospaced text with **no** highlight theme applied, and the call never
/// fails. A recognised language is coloured via `syntect`.
///
/// ```
/// use mewcode_client::runtime::view::highlight_code_block;
/// use ratatui::style::Style;
///
/// let lines = highlight_code_block("hello world\nsecond line", None);
/// assert_eq!(lines.len(), 2);
/// assert_eq!(lines[0].spans[0].style, Style::default());
///
/// let lines = highlight_code_block("x = 1", Some("totally-not-a-language"));
/// assert_eq!(lines[0].spans[0].style, Style::default());
/// ```
pub fn highlight_code_block(code: &str, lang: Option<&str>) -> Vec<Line<'static>> {
    let syntaxes = syntax_set();
    let syntax = lang
        .filter(|l| !l.is_empty())
        .and_then(|l| syntaxes.find_syntax_by_token(l));

    let Some(syntax) = syntax else {
        // Plain monospaced fallback — no theme, never fails.
        return plain_lines(code);
    };

    let mut highlighter = HighlightLines::new(syntax, theme());
    let mut out = Vec::new();
    for line in LinesWithEndings::from(code) {
        match highlighter.highlight_line(line, syntaxes) {
            Ok(ranges) => {
                let spans = ranges
                    .into_iter()
                    .map(|(style, text)| {
                        let c = style.foreground;
                        Span::styled(
                            text.trim_end_matches('\n').to_string(),
                            Style::default().fg(Color::Rgb(c.r, c.g, c.b)),
                        )
                    })
                    .collect::<Vec<_>>();
                out.push(Line::from(spans));
            }
            // A highlighter hiccup must never fail the render.
            Err(_) => out.push(Line::from(Span::raw(line.trim_end_matches('\n').to_string()))),
        }
    }
    out
}

/// Plain, un-themed lines (the code-block fallback).
fn plain_lines(code: &str) -> Vec<Line<'static>> {
    code.lines()
        .map(|l| Line::from(Span::raw(l.to_string())))
        .collect()
}

/// Deep-clone a borrowed [`Text`] into owned (`'static`) lines so it can
/// outlive the local string `tui-markdown` borrowed from.
fn own_lines(text: Text<'_>) -> Vec<Line<'static>> {
    text.lines
        .into_iter()
        .map(|line| {
            let spans = line
                .spans
                .into_iter()
                .map(|s| Span::styled(s.content.into_owned(), s.style))
                .collect::<Vec<_>>();
            let mut owned = Line::from(spans);
            owned.style = line.style;
            owned.alignment = line.alignment;
            owned
        })
        .collect()
}

/// The shared syntax set, loaded once.
fn syntax_set() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// The shared highlight theme, loaded once.
fn theme() -> &'static Theme {
    static THEMES: OnceLock<ThemeSet> = OnceLock::new();
    let set = THEMES.get_or_init(ThemeSet::load_defaults);
    // `base16-ocean.dark` ships with syntect's defaults.
    &set.themes["base16-ocean.dark"]
}

/// The `/tools` overlay body: every tool plus the total count.
fn tools_lines() -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = ToolName::ALL
        .iter()
        .map(|t| Line::from(format!("• {t}")))
        .collect();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("{} tools available", ToolName::ALL.len()),
        Style::default().fg(Color::DarkGray),
    )));
    lines
}

/// The `/skills` overlay body. The skill catalog is loaded by the engine at
/// runtime; the view shows whatever the model carries — here a hint until the
/// catalog is wired through.
fn skills_lines() -> Vec<Line<'static>> {
    vec![Line::from(Span::styled(
        "No skills loaded.",
        Style::default().fg(Color::DarkGray),
    ))]
}

/// Draw a centred, bordered overlay with a `Clear` underneath it.
fn render_overlay(frame: &mut Frame, area: Rect, title: &str, body: Vec<Line<'static>>) {
    let rect = centered_rect(area, 60, 60);
    frame.render_widget(Clear, rect);
    let block = Block::bordered()
        .title(format!(" {title}  (Esc to close) "))
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(Paragraph::new(Text::from(body)).block(block), rect);
}

/// A rectangle centred within `area`, sized as a percentage of it.
fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

/// The spinner glyph for a turn that has been running for `elapsed`.
///
/// The frame index is derived from elapsed time, so redrawing on each 50 ms
/// tick advances the spinner.
///
/// ```
/// use std::time::Duration;
/// use mewcode_client::runtime::view::spinner_frame;
///
/// let a = spinner_frame(Duration::from_millis(0));
/// let b = spinner_frame(Duration::from_millis(80));
/// assert_ne!(a, b); // a tick later, a different frame
/// ```
pub fn spinner_frame(elapsed: Duration) -> &'static str {
    let idx = (elapsed.as_millis() / 80) as usize % SPINNER_FRAMES.len();
    SPINNER_FRAMES[idx]
}

/// Opacity of a toast that has been showing for `elapsed`: fully opaque during
/// the hold window, then eased down to 0 over the fade window. The value
/// changes on each tick once fading begins, driving the animation.
///
/// ```
/// use std::time::Duration;
/// use mewcode_client::runtime::view::toast_alpha;
///
/// assert_eq!(toast_alpha(Duration::from_millis(0)), 1.0);
/// assert!(toast_alpha(Duration::from_millis(10_000)) <= 0.0);
/// ```
pub fn toast_alpha(elapsed: Duration) -> f32 {
    if elapsed <= TOAST_HOLD {
        return 1.0;
    }
    let into_fade = (elapsed - TOAST_HOLD).as_secs_f32();
    let fade = TOAST_FADE.as_secs_f32();
    if into_fade >= fade {
        return 0.0;
    }
    // Ease the fade with a sine-out curve.
    let progress = into_fade / fade;
    1.0 - (progress * std::f32::consts::FRAC_PI_2).sin()
}

/// Draw the active toast as a banner along the top of the screen, dimmed as it
/// fades.
fn render_toast(frame: &mut Frame, area: Rect, toast: &Toast) {
    let alpha = toast_alpha(toast.started_at.elapsed());
    if alpha <= 0.0 {
        return;
    }

    let base = match toast.kind {
        ToastKind::Error => Color::Red,
        ToastKind::Info => Color::Blue,
    };
    // As alpha drops, dim the toast toward the background by stepping to a
    // darker shade once it is mostly faded.
    let style = if alpha < 0.5 {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(base).add_modifier(Modifier::BOLD)
    };

    let width = (UnicodeWidthStr::width(toast.text.as_str()) as u16 + 4).min(area.width);
    let rect = Rect {
        x: area.x + area.width.saturating_sub(width),
        y: area.y,
        width,
        height: 1,
    };
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(Span::styled(format!(" {} ", toast.text), style)),
        rect,
    );
}
