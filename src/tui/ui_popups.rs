use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::tui::app::App;
use crate::tui::widgets::COMMANDS;
use crate::tui::widgets::ThinkingLevel;

fn popup_block(title: &str, theme_accent: Color, theme_muted: Color) -> Block<'static> {
    Block::default()
        .title(Span::styled(
            format!(" {} ", title),
            Style::default()
                .fg(theme_accent)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme_muted))
}

fn centered_popup(area: Rect, content_width: usize, content_height: usize) -> Rect {
    let width = (content_width as u16).min(area.width.saturating_sub(4));
    let height = (content_height as u16).min(area.height.saturating_sub(2));
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

pub fn draw_model_selector(frame: &mut Frame, app: &mut App) {
    let sel = &app.model_selector;
    if sel.filtered.is_empty() && sel.query.is_empty() {
        return;
    }

    let mut content_lines: Vec<Line<'static>> = Vec::new();
    let mut last_provider: Option<&str> = None;

    for (item_idx, &entry_idx) in sel.filtered.iter().enumerate() {
        let entry = &sel.entries[entry_idx];

        if last_provider != Some(&entry.provider) {
            if last_provider.is_some() {
                content_lines.push(Line::from(""));
            }
            content_lines.push(Line::from(Span::styled(
                format!("  {}", entry.provider),
                Style::default()
                    .fg(app.theme.muted_fg)
                    .add_modifier(Modifier::BOLD),
            )));
            last_provider = Some(&entry.provider);
        }

        let is_current = entry.provider == sel.current_provider && entry.model == sel.current_model;
        let is_sel = item_idx == sel.selected;

        let (prefix, marker_style) = if is_sel {
            ("\u{25b8} ", Style::default().fg(app.theme.accent))
        } else {
            ("  ", Style::default().fg(app.theme.muted_fg))
        };

        let name_style = if is_sel {
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD)
        } else if is_current {
            Style::default().fg(app.theme.accent)
        } else {
            Style::default().fg(Color::Reset)
        };

        let mut spans = vec![Span::styled(format!("  {}", prefix), marker_style)];
        spans.push(Span::styled(entry.model.clone(), name_style));

        content_lines.push(Line::from(spans));
    }

    if sel.filtered.is_empty() {
        content_lines.push(Line::from(Span::styled(" no matches", app.theme.dim)));
    }

    let search_line = format!(" /{}", sel.query);
    let footer = "\u{2191}\u{2193} select  enter confirm  esc cancel";
    let content_width = content_lines
        .iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(20)
        .max(footer.len() + 2)
        .max(search_line.len())
        + 4;
    let content_height = content_lines.len() + 4;

    let popup = centered_popup(frame.area(), content_width, content_height);
    app.layout.model_selector = Some(popup);

    frame.render_widget(Clear, popup);

    let block = popup_block("model", app.theme.accent, app.theme.muted_fg);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut all_lines: Vec<Line<'static>> = Vec::new();

    let search_display = if sel.query.is_empty() {
        Line::from(Span::styled(" type to filter\u{2026}", app.theme.dim))
    } else {
        Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::raw(sel.query.clone()),
            Span::styled("\u{258f}", Style::default().fg(app.theme.accent)),
        ])
    };
    all_lines.push(search_display);
    all_lines.push(Line::from(""));
    all_lines.extend(content_lines);
    all_lines.push(Line::from(""));
    all_lines.push(Line::from(Span::styled(
        format!(" {}", footer),
        app.theme.dim,
    )));

    frame.render_widget(Paragraph::new(all_lines), inner);
}

pub fn draw_agent_selector(frame: &mut Frame, app: &mut App) {
    let sel = &app.agent_selector;
    if sel.entries.is_empty() {
        return;
    }

    let mut content_lines: Vec<Line<'static>> = Vec::new();
    for (i, entry) in sel.entries.iter().enumerate() {
        let is_current = entry.name == sel.current;
        let is_sel = i == sel.selected;

        let (prefix, marker_style) = if is_sel {
            ("\u{25b8} ", Style::default().fg(app.theme.accent))
        } else {
            ("  ", Style::default())
        };

        let name_style = if is_sel {
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD)
        } else if is_current {
            Style::default().fg(app.theme.accent)
        } else {
            Style::default().fg(Color::Reset)
        };

        let desc_style = if is_sel {
            Style::default().fg(app.theme.accent)
        } else {
            app.theme.dim
        };

        let mut spans = vec![Span::styled(format!("  {}", prefix), marker_style)];
        spans.push(Span::styled(entry.name.clone(), name_style));
        spans.push(Span::styled(format!("  {}", entry.description), desc_style));

        content_lines.push(Line::from(spans));
    }

    let footer = "\u{2191}\u{2193} select  enter confirm  esc cancel";
    let content_width = content_lines
        .iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(20)
        .max(footer.len() + 2)
        + 4;
    let content_height = content_lines.len() + 3;

    let popup = centered_popup(frame.area(), content_width, content_height);

    frame.render_widget(Clear, popup);

    let block = popup_block("agent", app.theme.accent, app.theme.muted_fg);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut all_lines: Vec<Line<'static>> = Vec::new();
    all_lines.extend(content_lines);
    all_lines.push(Line::from(""));
    all_lines.push(Line::from(Span::styled(
        format!(" {}", footer),
        app.theme.dim,
    )));

    frame.render_widget(Paragraph::new(all_lines), inner);
}

pub fn draw_command_palette(frame: &mut Frame, app: &mut App, input_area: Rect) {
    let palette = &app.command_palette;
    if palette.filtered.is_empty() {
        app.layout.command_palette = None;
        return;
    }

    let items: Vec<(&str, &str)> = palette
        .filtered
        .iter()
        .map(|&i| (COMMANDS[i].name, COMMANDS[i].description))
        .collect();

    let width = items
        .iter()
        .map(|(n, d)| n.len() + d.len() + 8)
        .max()
        .unwrap_or(20) as u16;
    let height = items.len() as u16;

    let popup_y = input_area.y.saturating_sub(height + 1);
    let popup = Rect::new(
        input_area.x + 1,
        popup_y,
        width.min(input_area.width.saturating_sub(2)),
        height,
    );

    app.layout.command_palette = Some(popup);

    frame.render_widget(Clear, popup);

    let mut cmd_lines: Vec<Line<'static>> = Vec::new();
    for (i, (name, desc)) in items.iter().enumerate() {
        let is_sel = i == palette.selected;
        if is_sel {
            cmd_lines.push(Line::from(vec![
                Span::styled("\u{25b8} ", Style::default().fg(app.theme.accent)),
                Span::styled(
                    format!("/{:<10}", name),
                    Style::default()
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(desc.to_string(), Style::default().fg(app.theme.accent)),
            ]));
        } else {
            cmd_lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("/{:<10}", name), Style::default().fg(Color::Reset)),
                Span::styled(desc.to_string(), app.theme.dim),
            ]));
        }
    }

    frame.render_widget(Paragraph::new(cmd_lines), popup);
}

pub fn draw_empty_state(app: &App) -> Vec<Line<'static>> {
    let accent = app.theme.accent;
    let dim = app.theme.dim;
    vec![
        Line::from(""),
        Line::from(""),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "       \u{25c6}",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "       dot",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled("       start a conversation below", dim)),
        Line::from(Span::styled(
            "       /help \u{00b7} /model \u{00b7} /sessions",
            dim,
        )),
        Line::from(""),
    ]
}

pub fn draw_thinking_selector(frame: &mut Frame, app: &mut App) {
    let sel = &app.thinking_selector;
    if !sel.visible {
        return;
    }

    let levels = ThinkingLevel::all();
    let mut content_lines: Vec<Line<'static>> = Vec::new();

    for (i, &level) in levels.iter().enumerate() {
        let is_current = level == sel.current;
        let is_sel = i == sel.selected;

        let (prefix, marker_style) = if is_sel {
            ("\u{25b8} ", Style::default().fg(app.theme.accent))
        } else {
            ("  ", Style::default())
        };

        let name_style = if is_sel {
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD)
        } else if is_current {
            Style::default().fg(app.theme.accent)
        } else {
            Style::default().fg(Color::Reset)
        };

        let desc_style = if is_sel {
            Style::default().fg(app.theme.accent)
        } else {
            app.theme.dim
        };

        let mut spans = vec![Span::styled(format!("  {}", prefix), marker_style)];
        spans.push(Span::styled(level.label().to_string(), name_style));
        spans.push(Span::styled(
            format!("  {}", level.description()),
            desc_style,
        ));

        content_lines.push(Line::from(spans));
    }

    let footer = "\u{2191}\u{2193} select  enter confirm  esc cancel";
    let content_width = content_lines
        .iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(20)
        .max(footer.len() + 2)
        + 4;
    let content_height = content_lines.len() + 3;

    let popup = centered_popup(frame.area(), content_width, content_height);
    app.layout.thinking_selector = Some(popup);

    frame.render_widget(Clear, popup);

    let block = popup_block("thinking", app.theme.accent, app.theme.muted_fg);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut all_lines: Vec<Line<'static>> = Vec::new();
    all_lines.extend(content_lines);
    all_lines.push(Line::from(""));
    all_lines.push(Line::from(Span::styled(
        format!(" {}", footer),
        app.theme.dim,
    )));

    frame.render_widget(Paragraph::new(all_lines), inner);
}

pub fn draw_session_selector(frame: &mut Frame, app: &mut App) {
    let sel = &app.session_selector;
    if !sel.visible {
        return;
    }

    let mut content_lines: Vec<Line<'static>> = Vec::new();

    for (item_idx, &entry_idx) in sel.filtered.iter().enumerate() {
        let entry = &sel.entries[entry_idx];
        let is_sel = item_idx == sel.selected;

        let (prefix, title_style) = if is_sel {
            (
                "\u{25b8} ",
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            ("  ", Style::default().fg(Color::Reset))
        };

        let sub_style = if is_sel {
            Style::default().fg(app.theme.accent)
        } else {
            app.theme.dim
        };

        content_lines.push(Line::from(vec![
            Span::styled(
                format!("  {}", prefix),
                if is_sel {
                    Style::default().fg(app.theme.accent)
                } else {
                    Style::default()
                },
            ),
            Span::styled(entry.title.clone(), title_style),
            Span::styled(format!("  {}", entry.subtitle), sub_style),
        ]));
    }

    if sel.filtered.is_empty() && !sel.query.is_empty() {
        content_lines.push(Line::from(Span::styled(" no matches", app.theme.dim)));
    } else if sel.entries.is_empty() {
        content_lines.push(Line::from(Span::styled(
            " no sessions in this directory",
            app.theme.dim,
        )));
    }

    let footer = "\u{2191}\u{2193} select  enter resume  esc cancel";
    let content_width = content_lines
        .iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(30)
        .max(footer.len() + 2)
        + 4;
    let content_height = content_lines.len() + 4;

    let popup = centered_popup(frame.area(), content_width, content_height);
    app.layout.session_selector = Some(popup);

    frame.render_widget(Clear, popup);

    let block = popup_block("sessions", app.theme.accent, app.theme.muted_fg);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let search_display = if sel.query.is_empty() {
        Line::from(Span::styled(" type to filter\u{2026}", app.theme.dim))
    } else {
        Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::raw(sel.query.clone()),
            Span::styled("\u{258f}", Style::default().fg(app.theme.accent)),
        ])
    };

    let mut all_lines: Vec<Line<'static>> = Vec::new();
    all_lines.push(search_display);
    all_lines.push(Line::from(""));
    all_lines.extend(content_lines);
    all_lines.push(Line::from(""));
    all_lines.push(Line::from(Span::styled(
        format!(" {}", footer),
        app.theme.dim,
    )));

    frame.render_widget(Paragraph::new(all_lines), inner);
}
