//! The unified Workspace view — doc-faithful per `ui-aesthetic.md` §3.
//!
//! The Workspace is one screen with two regions stacked vertically:
//!
//! ```text
//! ┌── canvas region (top, 3 panes) ───────────────────────────┐
//! │ left rail │ canvas area │ status row                     │
//! ├───────────┴─────────── chat region (bottom) ──────────────┤
//! │ block strip (transcript + status)                        │
//! │ prompt editor                                            │
//! └──────────────────────────────────────────────────────────┘
//! ```
//!
//! In M1 the side rails are minimal: the left rail is a slim
//! summary (node count, selected id, focus indicator), the right
//! pane is a single status row. The C4 palette rail and the
//! properties inspector are M2 per `ui-aesthetic.md` §6.
//!
//! The chat region shows the transcript (with the existing
//! block-style role labels from the old session view), and the
//! prompt editor. When `ws.chat` is `None`, the transcript area
//! shows a "no session yet — type a prompt to start" hint and the
//! editor is still active; the first submit auto-creates a
//! session.
//!
//! Colors come from the `Theme` module — never hardcoded.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use mewcode_protocol::canvas::{Node, NodeKind};
use mewcode_protocol::{Message, MessagePart, Role};

use super::super::model::{CanvasState, Overlay, SessionState, WorkspaceFocus, WorkspaceState};
use super::markdown::render_markdown;
use super::overlay::{render_overlay, skills_lines, tools_lines};
use super::spinner::spinner_frame;
use super::theme::{Slot, style};

/// Height of the chat region (transcript + prompt + status),
/// in cells, when both are visible. The remaining cells go to
/// the canvas region.
const CHAT_HEIGHT: u16 = 12;
/// Height of the prompt editor inside the chat region.
const PROMPT_HEIGHT: u16 = 3;
/// Height of the chat status row.
const CHAT_STATUS_HEIGHT: u16 = 1;
/// Width of the left rail (the slim summary column).
const LEFT_RAIL_WIDTH: u16 = 18;
/// Height of the top status row (above the canvas, sits next to
/// the left rail).
const CANVAS_STATUS_HEIGHT: u16 = 1;

/// Draw the unified Workspace.
///
/// Takes `&mut WorkspaceState` because the chat renderer writes
/// `scroll` / `max_scroll` / `viewport` back during the draw (the
/// wrapped line count is only known once ratatui has wrapped the
/// text).
pub(super) fn render_workspace(frame: &mut Frame, area: Rect, ws: &mut WorkspaceState) {
    if area.height < CHAT_HEIGHT + 4 {
        // Too small for the layout. Show a single hint.
        frame.render_widget(
            Paragraph::new("terminal too small — resize to at least 16 rows")
                .style(style(Slot::FgDim)),
            area,
        );
        return;
    }

    // Split vertically: top (canvas region) | bottom (chat region).
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(CHAT_HEIGHT)])
        .split(area);

    render_canvas_region(frame, vert[0], ws);
    render_chat_region(frame, vert[1], ws);

    // Overlays (tools / skills) draw over the whole Workspace,
    // not just the chat region — they belong to the chat
    // region in spirit, but visually they look like Warp's
    // centered command palette.
    if let Some(s) = ws.chat.as_ref() {
        match s.overlay {
            Overlay::None => {}
            Overlay::Tools => render_overlay(frame, area, "Tools", tools_lines()),
            Overlay::Skills => render_overlay(frame, area, "Skills", skills_lines()),
        }
    }
}

fn render_canvas_region(frame: &mut Frame, area: Rect, ws: &WorkspaceState) {
    // Three-pane shell: left rail | canvas area | (right inspector omitted in M1).
    // The "inspector" pane is folded into a status row at the bottom of the
    // canvas area.
    let outer = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(LEFT_RAIL_WIDTH), Constraint::Min(1)])
        .split(area);

    render_left_rail(frame, outer[0], ws);

    let canvas_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(CANVAS_STATUS_HEIGHT)])
        .split(outer[1]);

    render_canvas_area(frame, canvas_area[0], &ws.canvas);
    render_canvas_status(frame, canvas_area[1], &ws.canvas, ws.focus);
}

fn render_left_rail(frame: &mut Frame, area: Rect, ws: &WorkspaceState) {
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(style(Slot::FgDim));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        " workspace ",
        style(Slot::Accent).add_modifier(ratatui::style::Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!(" {} nodes ", ws.canvas.graph.nodes.len()),
        style(Slot::FgDim),
    )));
    lines.push(Line::from(Span::styled(
        format!(" {} edges ", ws.canvas.graph.edges.len()),
        style(Slot::FgDim),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(" selected ", style(Slot::FgDim))));
    lines.push(Line::from(
        ws.canvas
            .selected
            .as_ref()
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|| "—".to_string()),
    ));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(" focus ", style(Slot::FgDim))));
    lines.push(Line::from(match ws.focus {
        WorkspaceFocus::Canvas => Span::styled("canvas", style(Slot::Accent)),
        WorkspaceFocus::Chat => Span::styled("chat", style(Slot::Accent)),
    }));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(" tab = swap ", style(Slot::FgDim))));
    lines.push(Line::from(Span::styled(" esc = home ", style(Slot::FgDim))));

    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn render_canvas_area(frame: &mut Frame, area: Rect, c: &CanvasState) {
    if c.loading {
        frame.render_widget(
            Paragraph::new("loading canvas from server…").style(style(Slot::FgDim)),
            area,
        );
        return;
    }
    if c.graph.nodes.is_empty() {
        let msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "No graph.json found.",
                style(Slot::Warning).add_modifier(ratatui::style::Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Drop a graph into .mewcode/canvas/graph.json and press Esc + c to reload.",
                style(Slot::FgDim),
            )),
        ])
        .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(msg, area);
        return;
    }

    let positions = c.resolved_positions();
    let viewport = c.viewport;

    for node in &c.graph.nodes {
        if let Some(&p) = positions.get(&node.id) {
            let (vx, vy) = (p.x - viewport.0, p.y - viewport.1);
            if vx + CanvasState::NODE_W <= 0 || vy + CanvasState::NODE_H <= 0 {
                continue;
            }
            if vx >= area.width as i32 || vy >= area.height as i32 {
                continue;
            }
            let rect = Rect {
                x: area.x.saturating_add(vx.max(0) as u16),
                y: area.y.saturating_add(vy.max(0) as u16),
                width: (CanvasState::NODE_W as u16)
                    .min(area.width.saturating_sub(vx.max(0) as u16)),
                height: (CanvasState::NODE_H as u16)
                    .min(area.height.saturating_sub(vy.max(0) as u16)),
            };
            if rect.width < 4 || rect.height < 2 {
                continue;
            }
            let is_selected = c.selected.as_ref().is_some_and(|s| s == &node.id);
            render_node(frame, rect, node, is_selected);
        }
    }

    for edge in &c.graph.edges {
        if let (Some(&a), Some(&b)) = (positions.get(&edge.from), positions.get(&edge.to)) {
            let av = mewcode_protocol::canvas::Point {
                x: a.x - viewport.0,
                y: a.y - viewport.1,
            };
            let bv = mewcode_protocol::canvas::Point {
                x: b.x - viewport.0,
                y: b.y - viewport.1,
            };
            render_edge(frame, area, av, bv);
        }
    }
}

fn render_node(frame: &mut Frame, rect: Rect, node: &Node, is_selected: bool) {
    let color = color_for_kind(node.kind);
    let title_style = if is_selected {
        style(Slot::Fg)
            .bg(color)
            .add_modifier(ratatui::style::Modifier::BOLD)
    } else {
        style(Slot::NodeSystem)
            .fg(color)
            .add_modifier(ratatui::style::Modifier::BOLD)
    };
    let rule = "─".repeat(rect.width.saturating_sub(2) as usize);
    let block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(style(Slot::Fg).fg(color))
        .title(Span::styled(format!(" {} ", node.name), title_style));
    let body = Line::from(Span::styled(rule, style(Slot::Fg).fg(color)));
    frame.render_widget(Paragraph::new(body).block(block), rect);
}

fn color_for_kind(kind: NodeKind) -> ratatui::style::Color {
    use super::theme::bg;
    match kind {
        NodeKind::System => bg(Slot::NodeSystem),
        NodeKind::Container => bg(Slot::NodeContainer),
        NodeKind::Component => bg(Slot::NodeComponent),
    }
}

fn render_edge(
    frame: &mut Frame,
    area: Rect,
    a: mewcode_protocol::canvas::Point,
    b: mewcode_protocol::canvas::Point,
) {
    let (left, right) = if a.x <= b.x { (a, b) } else { (b, a) };
    let source_bottom = left.y + CanvasState::NODE_H + 1;
    let target_bottom = right.y + CanvasState::NODE_H + 1;
    let col = area.x.saturating_add(left.x.max(0) as u16);
    if col >= area.x.saturating_add(area.width) {
        return;
    }
    let y_top = area.y.saturating_add(source_bottom.max(0) as u16);
    let y_bot = area.y.saturating_add(target_bottom.max(0) as u16);
    let area_bottom = area.y.saturating_add(area.height);
    let area_right = area.x.saturating_add(area.width).saturating_sub(1);
    for y in y_top.min(y_bot)..=y_top.max(y_bot) {
        if y < area_bottom {
            if let Some(cell) = frame.buffer_mut().cell_mut((col, y)) {
                cell.set_symbol("│");
                cell.set_style(style(Slot::Edge));
            }
        }
    }
    let y_draw = y_bot.min(area_bottom.saturating_sub(1));
    if y_draw < area.y {
        return;
    }
    let x_end = area.x.saturating_add(right.x.max(0) as u16);
    for x in col..=x_end.min(area_right) {
        if let Some(cell) = frame.buffer_mut().cell_mut((x, y_draw)) {
            let sym = if x == x_end { "▶" } else { "─" };
            cell.set_symbol(sym);
            cell.set_style(style(Slot::Edge));
        }
    }
}

fn render_canvas_status(frame: &mut Frame, area: Rect, c: &CanvasState, focus: WorkspaceFocus) {
    let focus_marker = if focus == WorkspaceFocus::Canvas {
        Span::styled(" ● ", style(Slot::Accent))
    } else {
        Span::styled("   ", style(Slot::Fg))
    };
    let text = if c.loading {
        "loading…".to_string()
    } else if let Some(id) = &c.selected {
        format!(
            "esc home · tab swap · ↑↓←→ move · drag pan · selected: {}",
            id.as_str()
        )
    } else {
        "esc home · tab swap · ↑↓←→ move · drag pan · click select".to_string()
    };
    let line = Line::from(vec![focus_marker, Span::styled(text, style(Slot::FgDim))]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_chat_region(frame: &mut Frame, area: Rect, ws: &mut WorkspaceState) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(style(Slot::FgDim));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(PROMPT_HEIGHT),
            Constraint::Length(CHAT_STATUS_HEIGHT),
        ])
        .split(inner);

    render_transcript(frame, chunks[0], ws);
    render_prompt(frame, chunks[1], ws);
    render_chat_status(frame, chunks[2], ws);
}

fn render_transcript(frame: &mut Frame, area: Rect, ws: &mut WorkspaceState) {
    let mut lines: Vec<Line> = Vec::new();
    if let Some(s) = ws.chat.as_mut() {
        for msg in &s.session.messages {
            lines.extend(render_message(msg));
            lines.push(Line::from(""));
        }
        if let Some(st) = &s.streaming {
            lines.push(Line::from(Span::styled(
                format!("{} assistant", spinner_frame(st.started_at.elapsed())),
                style(Slot::Warning),
            )));
            if !st.buffer.is_empty() {
                lines.extend(render_markdown(&st.buffer));
            }
        }
    } else {
        // No session yet. Show a one-line hint.
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "no session yet — type a prompt to start one.",
            style(Slot::FgDim),
        )));
    }

    let para = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
    let total = para.line_count(area.width).min(u16::MAX as usize) as u16;

    // The scroll bookkeeping lives on the chat state. If the
    // session is missing, there's nothing to scroll.
    if let Some(s) = ws.chat.as_mut() {
        s.viewport = area.height;
        s.max_scroll = total.saturating_sub(area.height);
        if s.follow {
            s.scroll = s.max_scroll;
        } else {
            s.scroll = s.scroll.min(s.max_scroll);
        }
        frame.render_widget(para.scroll((s.scroll, 0)), area);
    } else {
        frame.render_widget(para, area);
    }
}

fn render_prompt(frame: &mut Frame, area: Rect, ws: &WorkspaceState) {
    let (input_text, focused) = match ws.chat.as_ref() {
        Some(s) => (s.input.lines().join("\n"), ws.focus == WorkspaceFocus::Chat),
        None => (String::new(), ws.focus == WorkspaceFocus::Chat),
    };
    let title = if focused {
        " ▸ message "
    } else {
        "   message "
    };
    let border = if focused {
        style(Slot::Accent)
    } else {
        style(Slot::FgDim)
    };
    let input = Paragraph::new(input_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(border),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(input, area);
}

fn render_chat_status(frame: &mut Frame, area: Rect, ws: &WorkspaceState) {
    let focus_marker = if ws.focus == WorkspaceFocus::Chat {
        Span::styled(" ● ", style(Slot::Accent))
    } else {
        Span::styled("   ", style(Slot::Fg))
    };
    let text = match ws.chat.as_ref() {
        Some(s) => {
            if s.streaming.is_some() {
                format!("{}  •  streaming…", s.session.model.display_name(),)
            } else {
                format!(
                    "{}  •  PgUp/PgDn scroll  •  /tools  /skills",
                    s.session.model.display_name(),
                )
            }
        }
        None => "tab = canvas · esc = home".to_string(),
    };
    let line = Line::from(vec![focus_marker, Span::styled(text, style(Slot::FgDim))]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_message(msg: &Message) -> Vec<Line<'static>> {
    use ratatui::style::Modifier;
    let (label, label_style) = match msg.role {
        Role::User => ("you", style(Slot::Success).add_modifier(Modifier::BOLD)),
        Role::Assistant => (
            "assistant",
            style(Slot::Accent).add_modifier(Modifier::BOLD),
        ),
        Role::Tool => ("tool", style(Slot::Warning).add_modifier(Modifier::BOLD)),
    };
    let mut out = vec![Line::from(Span::styled(label.to_string(), label_style))];
    for part in &msg.parts {
        match part {
            MessagePart::Text { text } => out.extend(render_markdown(text)),
            MessagePart::ToolCall(call) => out.push(Line::from(Span::styled(
                format!("→ {}({})", call.name, call.input),
                style(Slot::Warning),
            ))),
            MessagePart::ToolResult(res) => out.push(Line::from(Span::styled(
                format!("← {} result", res.name),
                style(Slot::Warning),
            ))),
            MessagePart::FileMention { path } => out.push(Line::from(Span::styled(
                format!("@{path}"),
                style(Slot::AccentAlt),
            ))),
        }
    }
    out
}

// Keep SessionState referenced so the import doesn't get
// dropped by an over-aggressive linter.
#[allow(dead_code)]
fn _typecheck(_: &SessionState) {}
