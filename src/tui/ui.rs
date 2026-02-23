use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};

use crate::tui::app::{App, AppMode};
use crate::tui::markdown;
use crate::tui::ui_popups;
use crate::tui::ui_tools;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let input_height = app.input_height();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(input_height),
            Constraint::Length(1),
        ])
        .split(frame.area());

    app.layout.header = chunks[0];
    app.layout.messages = chunks[1];
    app.layout.input = chunks[2];
    app.layout.status = chunks[3];

    draw_header(frame, app, chunks[0]);
    draw_messages(frame, app, chunks[1]);
    draw_input(frame, app, chunks[2]);
    draw_status(frame, app, chunks[3]);

    if app.model_selector.visible {
        ui_popups::draw_model_selector(frame, app);
    }

    if app.agent_selector.visible {
        ui_popups::draw_agent_selector(frame, app);
    }

    if app.thinking_selector.visible {
        ui_popups::draw_thinking_selector(frame, app);
    }

    if app.command_palette.visible {
        ui_popups::draw_command_palette(frame, app, chunks[2]);
    }

    if app.session_selector.visible {
        ui_popups::draw_session_selector(frame, app);
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let sep = Span::styled(" \u{00b7} ", app.theme.separator);

    let title_text = app
        .conversation_title
        .as_deref()
        .unwrap_or("new conversation");

    let mut left_spans = vec![Span::styled(
        format!(" {}", title_text),
        Style::default().fg(app.theme.accent),
    )];

    let show_agent = !app.agent_name.is_empty()
        && app.agent_name != "default"
        && app.agent_name != "dot";
    if show_agent {
        left_spans.push(sep.clone());
        left_spans.push(Span::styled(
            format!("@{}", app.agent_name),
            app.theme.status_bar,
        ));
    }

    let left_width: usize = left_spans.iter().map(|s| s.content.chars().count()).sum();

    let model_short = shorten_model(&app.model_name);
    let mut right_spans: Vec<Span<'static>> = Vec::new();

    right_spans.push(Span::styled(model_short, app.theme.dim));

    if app.thinking_budget > 0 {
        right_spans.push(sep.clone());
        right_spans.push(Span::styled(
            format!("\u{25c7}{}", app.thinking_level().label()),
            app.theme.thinking,
        ));
    }

    right_spans.push(Span::raw(" "));
    right_spans.push(match app.mode {
        AppMode::Normal => Span::styled(
            " NORMAL ",
            Style::default()
                .fg(app.theme.mode_normal_fg)
                .bg(app.theme.mode_normal_bg),
        ),
        AppMode::Insert => Span::styled(
            " INSERT ",
            Style::default()
                .fg(app.theme.mode_insert_fg)
                .bg(app.theme.mode_insert_bg),
        ),
    });

    if let Some(elapsed) = app.streaming_elapsed_secs() {
        right_spans.push(Span::styled(
            format!(" {}", format_elapsed(elapsed)),
            app.theme.thinking,
        ));
    }

    let right_width: usize = right_spans.iter().map(|s| s.content.chars().count()).sum();
    let gap = (area.width as usize).saturating_sub(left_width + right_width + 1);

    let mut spans = left_spans;
    spans.push(Span::raw(" ".repeat(gap)));
    spans.extend(right_spans);

    let header = Line::from(spans);
    frame.render_widget(Paragraph::new(header), area);
}

fn draw_messages(frame: &mut Frame, app: &mut App, area: Rect) {
    let inner = Rect {
        x: area.x + 1,
        y: area.y,
        width: area.width.saturating_sub(3),
        height: area.height,
    };

    let mut all_lines: Vec<Line<'static>> = Vec::new();

    for msg in &app.messages {
        all_lines.push(Line::from(""));

        if msg.role == "user" {
            let mut content_lines = msg.content.lines();
            if let Some(first) = content_lines.next() {
                all_lines.push(Line::from(vec![
                    Span::styled("  \u{25cf} ", Style::default().fg(app.theme.muted_fg)),
                    Span::styled(first.to_string(), app.theme.user_text),
                ]));
            }
            for text_line in content_lines {
                all_lines.push(Line::from(Span::styled(
                    format!("    {}", text_line),
                    app.theme.user_text,
                )));
            }
        } else if msg.role == "compact" {
            all_lines.push(Line::from(vec![
                Span::styled("  ", app.theme.thinking),
                Span::styled(msg.content.clone(), app.theme.dim),
            ]));
        } else {
            let model_label = msg
                .model
                .as_deref()
                .map(|m| shorten_model(m))
                .unwrap_or_default();
            if model_label.is_empty() {
                all_lines.push(Line::from(Span::styled(
                    "  \u{25c6}",
                    Style::default().fg(app.theme.accent),
                )));
            } else {
                all_lines.push(Line::from(vec![
                    Span::styled(
                        "  \u{25c6} ",
                        Style::default().fg(app.theme.accent),
                    ),
                    Span::styled(model_label, app.theme.dim),
                ]));
            }
            if let Some(ref thinking) = msg.thinking {
                render_thinking_block(thinking, app.thinking_expanded, &app.theme, &mut all_lines);
            }
            let md_lines =
                markdown::render_markdown(&msg.content, &app.theme, inner.width.saturating_sub(4));
            for line in md_lines {
                let mut padded = vec![Span::raw("    ")];
                padded.extend(line.spans);
                all_lines.push(Line::from(padded));
            }
            ui_tools::render_tool_calls(&msg.tool_calls, &app.theme, &mut all_lines);
        }
    }

    if app.is_streaming {
        ui_tools::render_streaming_state(app, inner.width, &mut all_lines);
    }

    if let Some(ref err) = app.error_message {
        all_lines.push(Line::from(""));
        all_lines.push(Line::from(Span::styled(
            format!("    {}", err),
            app.theme.dim,
        )));
    }

    if all_lines.is_empty() {
        all_lines.extend(ui_popups::draw_empty_state(app));
    }

    let total_visual: u32 = all_lines
        .iter()
        .map(|l| {
            if inner.width == 0 {
                return 1u32;
            }
            let chars: usize = l.spans.iter().map(|s| s.content.chars().count()).sum();
            if chars == 0 {
                1
            } else {
                (chars as u32).div_ceil(inner.width as u32).max(1)
            }
        })
        .sum();
    let visible = inner.height as u32;
    app.max_scroll = total_visual.saturating_sub(visible).min(u16::MAX as u32) as u16;
    if app.follow_bottom || app.scroll_offset > app.max_scroll {
        app.scroll_offset = app.max_scroll;
    }

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(app.theme.border);

    let paragraph = Paragraph::new(all_lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));

    frame.render_widget(paragraph, area);

    if app.max_scroll > 0 {
        let scrollbar_area = Rect {
            x: area.x + area.width.saturating_sub(1),
            y: area.y + 1,
            width: 1,
            height: area.height.saturating_sub(1),
        };
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .track_symbol(Some("\u{2502}"))
            .thumb_symbol("\u{2588}")
            .begin_symbol(None)
            .end_symbol(None)
            .track_style(app.theme.scrollbar_track)
            .thumb_style(app.theme.scrollbar_thumb);

        let mut state =
            ScrollbarState::new(app.max_scroll as usize).position(app.scroll_offset as usize);
        frame.render_stateful_widget(scrollbar, scrollbar_area, &mut state);
    }
}

fn render_thinking_block(
    thinking: &str,
    expanded: bool,
    theme: &crate::tui::theme::Theme,
    lines: &mut Vec<Line<'static>>,
) {
    let word_count = thinking.split_whitespace().count();
    if expanded {
        lines.push(Line::from(vec![
            Span::styled("    \u{25be} ", theme.thinking),
            Span::styled(
                "thinking",
                Style::default()
                    .fg(theme.muted_fg)
                    .add_modifier(Modifier::ITALIC),
            ),
            Span::styled(format!(" \u{00b7} {}w", word_count), theme.dim),
        ]));
        for text_line in thinking.lines() {
            lines.push(Line::from(vec![
                Span::styled("    \u{2502} ", theme.thinking),
                Span::styled(
                    text_line.to_string(),
                    Style::default()
                        .fg(theme.muted_fg)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
        }
        lines.push(Line::from(Span::styled("    ", theme.thinking)));
    } else {
        let hint = if word_count > 10 {
            format!(" \u{00b7} {}w", word_count)
        } else {
            String::new()
        };
        lines.push(Line::from(vec![
            Span::styled("    \u{25b8} ", theme.thinking),
            Span::styled(
                "thinking",
                Style::default()
                    .fg(theme.muted_fg)
                    .add_modifier(Modifier::ITALIC),
            ),
            Span::styled(hint, theme.dim),
            Span::styled("  [t]", theme.dim),
        ]));
    }
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let is_active = app.mode == AppMode::Insert && !app.is_streaming;

    let border_style = if is_active {
        Style::default().fg(app.theme.accent)
    } else {
        app.theme.border
    };

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(border_style);

    let inner = block.inner(area);

    let (prompt, prompt_style) = if is_active {
        (
            "\u{25c6} ",
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        ("\u{25c7} ", Style::default().fg(app.theme.muted_fg))
    };

    let display_lines: Vec<Line<'static>> = if app.is_streaming {
        let pulse = ["\u{25c7}", "\u{25c6}"];
        let idx = (app.tick_count / 20 % pulse.len() as u64) as usize;

        let dot_count = ((app.tick_count / 16) % 4) as usize;
        let dots: String = ".".repeat(dot_count);

        let mut spans = vec![
            Span::styled(
                format!("  {} ", pulse[idx]),
                Style::default().fg(app.theme.accent),
            ),
            Span::styled(format!("generating{:<3}", dots), app.theme.dim),
        ];
        if let Some(elapsed) = app.streaming_elapsed_secs() {
            spans.push(Span::styled(
                format!(" \u{00b7} {}", format_elapsed(elapsed)),
                app.theme.dim,
            ));
        }
        vec![Line::from(spans)]
    } else if app.input.is_empty() && app.attachments.is_empty() {
        vec![Line::from(vec![
            Span::styled(prompt, prompt_style),
            Span::styled("message or /help", app.theme.dim),
        ])]
    } else {
        let mut lines = Vec::new();
        if !app.attachments.is_empty() {
            let att_display: Vec<String> = app
                .attachments
                .iter()
                .map(|a| {
                    std::path::Path::new(&a.path)
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| a.path.clone())
                })
                .collect();
            lines.push(Line::from(vec![
                Span::styled(prompt, prompt_style),
                Span::styled(
                    format!("\u{1f4ce} {}", att_display.join(", ")),
                    app.theme.dim,
                ),
            ]));
        }

        let display = app.display_input();
        if display.is_empty() && !app.attachments.is_empty() {
            if lines.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled(prompt, prompt_style),
                    Span::styled("add a message or press enter", app.theme.dim),
                ]));
            }
        } else {
            let offset = if app.attachments.is_empty() { 0 } else { 1 };
            for (i, line) in display.lines().enumerate() {
                if i == 0 && offset == 0 {
                    lines.push(Line::from(vec![
                        Span::styled(prompt, prompt_style),
                        Span::raw(line.to_string()),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::raw(line.to_string()),
                    ]));
                }
            }
            if display.ends_with('\n') {
                lines.push(Line::from(Span::raw("  ")));
            }
        }
        lines
    };

    let paragraph = Paragraph::new(display_lines).wrap(Wrap { trim: false });

    frame.render_widget(block, area);
    frame.render_widget(paragraph, inner);

    if is_active && !app.model_selector.visible {
        let blink_on = (app.tick_count / 32) % 2 == 0;
        if blink_on {
            let (cx, cy) = cursor_position(&app.input, app.cursor_pos, inner);
            if cy < inner.y + inner.height {
                frame.set_cursor_position((cx, cy));
            }
        }
    }
}

fn cursor_position(input: &str, byte_pos: usize, area: Rect) -> (u16, u16) {
    let before = &input[..byte_pos.min(input.len())];
    let mut row: u16 = 0;
    let mut col: u16 = 2;

    for ch in before.chars() {
        if ch == '\n' {
            row += 1;
            col = 2;
        } else {
            col += 1;
        }
    }

    (area.x + col, area.y + row)
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let mut left_spans: Vec<Span<'static>> = vec![Span::styled(
        format!(
            " {}in \u{00b7} {}out",
            format_tokens(app.usage.input_tokens),
            format_tokens(app.usage.output_tokens),
        ),
        app.theme.status_bar,
    )];

    if app.usage.total_cost > 0.0 {
        left_spans.push(Span::styled(
            format!(" \u{00b7} ${:.2}", app.usage.total_cost),
            app.theme.cost,
        ));
    }

    let left_width: usize = left_spans.iter().map(|s| s.content.chars().count()).sum();

    let scroll_indicator = if app.max_scroll > 0 {
        let pct = if app.max_scroll == 0 {
            100
        } else {
            (app.scroll_offset as u32 * 100 / app.max_scroll as u32).min(100)
        };
        format!("{}% ", pct)
    } else {
        String::new()
    };

    let hint = if app.model_selector.visible
        || app.agent_selector.visible
        || app.thinking_selector.visible
    {
        "\u{2191}\u{2193} enter esc "
    } else if app.mode == AppMode::Insert {
        "/help "
    } else {
        "i j/k q "
    };

    let right_width = scroll_indicator.len() + hint.len();
    let padding = (area.width as usize).saturating_sub(left_width + right_width);

    let mut line_spans = left_spans;
    line_spans.push(Span::raw(" ".repeat(padding)));
    line_spans.push(Span::styled(scroll_indicator, app.theme.dim));
    line_spans.push(Span::styled(hint.to_string(), app.theme.dim));

    frame.render_widget(Paragraph::new(Line::from(line_spans)), area);
}

pub fn format_elapsed(secs: f64) -> String {
    if secs < 1.0 {
        "<1s".to_string()
    } else if secs < 60.0 {
        format!("{:.0}s", secs)
    } else {
        let m = (secs / 60.0).floor() as u32;
        let s = (secs % 60.0).floor() as u32;
        format!("{}m{}s", m, s)
    }
}

pub fn shorten_model(model: &str) -> String {
    if model.len() <= 30 {
        return model.to_string();
    }
    if let Some(idx) = model.rfind('-') {
        let suffix = &model[idx..];
        if suffix.len() > 8 {
            return format!("{}{}", &model[..25], "\u{2026}");
        }
    }
    format!("{}\u{2026}", &model[..29])
}

fn format_tokens(n: u32) -> String {
    if n >= 1_000_000 {
        format!("{:.1}m", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
