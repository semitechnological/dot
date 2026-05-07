use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Color;
use ratatui::text::Line;

use crate::agent::TodoStatus;
use crate::tui::app::{App, ChatMessage};
use crate::tui::markdown;
use crate::tui::ui_popups;
use crate::tui::ui_tools;

const SHELL_TEMPLATE: &str = include_str!("crepus/shell.crepus");

fn normalize_crepus_text(raw: &str) -> String {
    raw.replace(['\n', '\r'], " ")
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('{', "｛")
        .replace('}', "｝")
}

fn row_ctx(line: &Line<'_>) -> crepuscularity_tui::TemplateContext {
    let text: String = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect();
    let color = line.spans.iter().find_map(|span| span.style.fg);
    let mut ctx = crepuscularity_tui::TemplateContext::new();
    ctx.set("text", normalize_crepus_text(&text));
    ctx.set(
        "muted",
        matches!(color, Some(Color::Indexed(8)) | Some(Color::DarkGray)),
    );
    ctx.set("accent", matches!(color, Some(Color::Rgb(137, 180, 250))));
    ctx.set("error", matches!(color, Some(Color::Rgb(243, 139, 168))));
    ctx
}

fn build_crepus_shell_context(app: &mut App, width: u16) -> crepuscularity_tui::TemplateContext {
    let title = normalize_crepus_text(
        app.conversation_title
            .as_deref()
            .unwrap_or("new conversation"),
    );
    let model = normalize_crepus_text(&display_model(&app.model_name));
    let status = normalize_crepus_text(
        app.status_message
            .as_ref()
            .map(|s| s.text.as_str())
            .unwrap_or(""),
    );
    let input = normalize_crepus_text(&app.input);

    let model_width = model.chars().count().max(1);
    let status_width = status
        .chars()
        .count()
        .min(width.saturating_sub(4) as usize)
        .max(1);

    rebuild_render_cache(app);
    let rows: Vec<crepuscularity_tui::TemplateContext> = app
        .render_cache
        .as_ref()
        .map(|cache| {
            let start = app.scroll_offset as usize;
            let end = (start + app.layout.messages.height as usize).min(cache.lines.len());
            cache.lines[start..end].iter().map(row_ctx).collect()
        })
        .unwrap_or_default();

    let todos: Vec<crepuscularity_tui::TemplateContext> = app
        .todos
        .iter()
        .take(5)
        .map(|todo| {
            let mut ctx = crepuscularity_tui::TemplateContext::new();
            ctx.set("text", normalize_crepus_text(&todo.content));
            ctx.set("active", todo.status == TodoStatus::InProgress);
            ctx.set("done", todo.status == TodoStatus::Completed);
            ctx
        })
        .collect();
    let subagents: Vec<crepuscularity_tui::TemplateContext> = app
        .background_subagents
        .iter()
        .take(4)
        .map(|subagent| {
            let mut ctx = crepuscularity_tui::TemplateContext::new();
            ctx.set("description", normalize_crepus_text(&subagent.description));
            ctx.set("done", subagent.done);
            ctx.set(
                "detail",
                normalize_crepus_text(
                    subagent
                        .current_tool
                        .as_deref()
                        .or(subagent.current_tool_detail.as_deref())
                        .unwrap_or(""),
                ),
            );
            ctx
        })
        .collect();

    let mut ctx = crepuscularity_tui::TemplateContext::new();
    ctx.set("title", title);
    ctx.set("model", model);
    ctx.set("model_width", model_width as i64);
    ctx.set("status", status);
    ctx.set("status_width", status_width as i64);
    ctx.set("input", input);
    ctx.set(
        "tokens",
        format!(
            "{}in · {}out",
            format_tokens(app.usage.input_tokens),
            format_tokens(app.usage.output_tokens)
        ),
    );
    ctx.set("rows", crepuscularity_tui::TemplateValue::List(rows));
    ctx.set(
        "todos",
        crepuscularity_tui::TemplateValue::List(todos.clone()),
    );
    ctx.set("has_todos", !todos.is_empty());
    ctx.set("todos_height", (todos.len() + 2).min(7) as i64);
    ctx.set(
        "subagents",
        crepuscularity_tui::TemplateValue::List(subagents.clone()),
    );
    ctx.set("has_subagents", !subagents.is_empty());
    ctx.set("subagents_height", (subagents.len() + 2).min(6) as i64);
    ctx
}

fn rebuild_render_cache(app: &mut App) {
    let width = app.layout.messages.width;
    if width == 0 {
        app.render_cache = None;
        app.max_scroll = 0;
        app.scroll_offset = 0;
        return;
    }

    let need = app.render_dirty
        || app.render_cache.as_ref().map(|cache| cache.width) != Some(width)
        || app.is_streaming;
    if !need {
        update_scroll_state(app);
        return;
    }

    let mut lines = Vec::new();
    let mut line_to_msg = Vec::new();
    let mut line_to_tool = Vec::new();

    for (idx, msg) in app.messages.iter().enumerate() {
        render_message_rows(
            app,
            msg,
            idx,
            width,
            &mut lines,
            &mut line_to_msg,
            &mut line_to_tool,
        );
    }

    let tail = lines.len();
    if app.is_streaming {
        let idx = app.messages.len();
        ui_tools::render_streaming_state(app, width, &mut lines, &mut line_to_tool, idx);
        line_to_msg.extend(std::iter::repeat_n(idx, lines.len() - tail));
    }

    if lines.is_empty() {
        lines.extend([
            Line::from(""),
            Line::from("dot"),
            Line::from("type a message to begin"),
        ]);
        line_to_msg.extend([0, 0, 0]);
        line_to_tool.extend([None, None, None]);
    }

    let (lines, line_to_msg, line_to_tool) =
        pre_wrap_lines(lines, line_to_msg, line_to_tool, width);
    let total_visual = lines.len() as u32;
    app.content_width = width;
    app.message_line_map.clone_from(&line_to_msg);
    app.tool_line_map.clone_from(&line_to_tool);
    app.render_cache = Some(crate::tui::app::RenderCache {
        lines,
        line_to_msg,
        line_to_tool,
        total_visual,
        width,
        wrap_heights: Vec::new(),
    });
    app.render_dirty = false;
    update_scroll_state(app);
}

fn render_message_rows(
    app: &App,
    msg: &ChatMessage,
    idx: usize,
    width: u16,
    lines: &mut Vec<Line<'static>>,
    line_to_msg: &mut Vec<usize>,
    line_to_tool: &mut Vec<Option<(usize, usize)>>,
) {
    if !lines.is_empty() {
        lines.push(Line::from(""));
        line_to_msg.push(idx);
        line_to_tool.push(None);
    }

    lines.push(Line::from(msg.role.clone()));
    line_to_msg.push(idx);
    line_to_tool.push(None);

    let content = if msg.content.trim().is_empty() {
        "(empty)"
    } else {
        msg.content.trim()
    };
    let before = lines.len();
    lines.extend(markdown::render_markdown(content, &app.theme, width));
    let added = lines.len() - before;
    line_to_msg.extend(std::iter::repeat_n(idx, added));
    line_to_tool.extend(std::iter::repeat_n(None, added));

    if !msg.tool_calls.is_empty() {
        ui_tools::render_tool_calls_compact(
            ui_tools::RenderToolCallsParams {
                tool_calls: &msg.tool_calls,
                theme: &app.theme,
                compact: width < 55,
                lines,
                line_to_tool: Some(line_to_tool),
                msg_idx: idx,
                width,
                tool_idx_base: 0,
            },
            |tool| app.expanded_tool_calls.contains(&(idx, tool)),
        );
        line_to_msg.extend(std::iter::repeat_n(idx, lines.len() - before - added));
    }
}

fn update_scroll_state(app: &mut App) {
    let total = app
        .render_cache
        .as_ref()
        .map(|cache| cache.total_visual)
        .unwrap_or(0);
    let visible = app.layout.messages.height as u32;
    app.max_scroll = total.saturating_sub(visible);
    if app.follow_bottom || app.scroll_offset > app.max_scroll {
        app.scroll_offset = app.max_scroll;
    }
}

type PreWrapResult = (Vec<Line<'static>>, Vec<usize>, Vec<Option<(usize, usize)>>);

fn pre_wrap_lines(
    lines: Vec<Line<'static>>,
    line_to_msg: Vec<usize>,
    line_to_tool: Vec<Option<(usize, usize)>>,
    width: u16,
) -> PreWrapResult {
    use unicode_width::UnicodeWidthChar;
    if width == 0 {
        return (lines, line_to_msg, line_to_tool);
    }
    let w = width as usize;
    let mut out = Vec::with_capacity(lines.len());
    let mut out_msg = Vec::with_capacity(lines.len());
    let mut out_tool = Vec::with_capacity(lines.len());

    for (i, line) in lines.into_iter().enumerate() {
        let msg = line_to_msg.get(i).copied().unwrap_or(0);
        let tool = line_to_tool.get(i).copied().flatten();
        if line.width() <= w {
            out.push(line);
            out_msg.push(msg);
            out_tool.push(tool);
            continue;
        }
        let mut row = Vec::new();
        let mut len = 0usize;
        for span in line.spans {
            let style = span.style;
            let text = span.content.to_string();
            let mut start = 0;
            for (byte, ch) in text.char_indices() {
                let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
                if len + cw > w && len > 0 {
                    if start < byte {
                        row.push(ratatui::text::Span::styled(
                            text[start..byte].to_string(),
                            style,
                        ));
                    }
                    out.push(Line::from(std::mem::take(&mut row)));
                    out_msg.push(msg);
                    out_tool.push(tool);
                    len = 0;
                    start = byte;
                }
                len += cw;
            }
            if start < text.len() {
                row.push(ratatui::text::Span::styled(
                    text[start..].to_string(),
                    style,
                ));
            }
        }
        if !row.is_empty() {
            out.push(Line::from(row));
            out_msg.push(msg);
            out_tool.push(tool);
        }
    }
    (out, out_msg, out_tool)
}

fn draw_shell_crepus(frame: &mut Frame, app: &mut App) {
    let ctx = build_crepus_shell_context(app, frame.area().width);
    if let Err(err) = crepuscularity_tui::render_template(SHELL_TEMPLATE, &ctx, frame, frame.area())
    {
        app.status_message = Some(crate::tui::app::StatusMessage::error(format!(
            "crepus-ui render error: {err}"
        )));
    }
    let display = app.display_input();
    let dcursor = app.display_cursor_pos();
    let (cx, cy) = cursor_position(&display, dcursor, app.layout.input);
    if cy < frame.area().y + frame.area().height {
        frame.set_cursor_position((cx, cy));
    }
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    setup_layout(frame, app);
    draw_shell_crepus(frame, app);
    draw_crepus_overlays(frame, app);
}

fn draw_crepus_overlays(frame: &mut Frame, app: &mut App) {
    if app.welcome_screen.visible {
        ui_popups::draw_welcome_screen(frame, app);
    }

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
        ui_popups::draw_command_palette(frame, app, app.layout.input);
    }

    if app.file_picker.visible {
        ui_popups::draw_file_picker(frame, app, app.layout.input);
    }

    if app.session_selector.visible {
        ui_popups::draw_session_selector(frame, app);
    }

    if app.help_popup.visible {
        ui_popups::draw_help_popup(frame, app);
    }

    if app.context_menu.visible {
        ui_popups::draw_context_menu(frame, app);
    }

    if app.pending_question.is_some() {
        ui_popups::draw_question_popup(frame, app);
    }

    if app.pending_permission.is_some() {
        ui_popups::draw_permission_popup(frame, app);
    }

    if app.rename_visible {
        ui_popups::draw_rename_popup(frame, app);
    }

    if app.login_popup.visible {
        ui_popups::draw_login_popup(frame, app);
    }

    if app.aside_popup.visible {
        ui_popups::draw_aside_popup(frame, app);
    }
}

fn setup_layout(frame: &Frame, app: &mut App) -> [Rect; 6] {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(0),
            Constraint::Length(2),
            Constraint::Length(0),
            Constraint::Length(2),
        ])
        .areas(frame.area());

    app.layout.header = chunks[0];
    app.layout.messages = chunks[1];
    app.layout.input = Rect {
        x: chunks[3].x.saturating_add(1),
        y: chunks[3].y.saturating_add(1),
        width: chunks[3].width.saturating_sub(1),
        height: 1,
    };
    app.layout.status = chunks[5];
    chunks
}

fn cursor_position(input: &str, byte_pos: usize, area: Rect) -> (u16, u16) {
    let before = &input[..byte_pos.min(input.len())];
    let width = area.width as usize;
    if width == 0 {
        return (area.x, area.y);
    }
    let prefix_w: usize = 2;
    let mut visual_row: usize = 0;
    let mut segments = before.split('\n').peekable();
    while let Some(seg) = segments.next() {
        let char_count = seg.chars().count();
        let total = prefix_w + char_count;
        if segments.peek().is_none() {
            let extra_rows = total / width;
            let col = total % width;
            visual_row += extra_rows;
            return (area.x + col as u16, area.y + visual_row as u16);
        }
        let rows = if total == 0 { 1 } else { total.div_ceil(width) };
        visual_row += rows;
    }
    (area.x + prefix_w as u16, area.y + visual_row as u16)
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

pub fn display_model(model: &str) -> String {
    let formatted = format_model_name(model);
    if formatted.chars().count() <= 30 {
        return formatted;
    }
    let truncated: String = formatted.chars().take(29).collect();
    format!("{}\u{2026}", truncated)
}

fn format_model_name(model: &str) -> String {
    let base = if let Some(pos) = model.rfind('-') {
        let suffix = &model[pos + 1..];
        if suffix.len() == 8 && suffix.chars().all(|c| c.is_ascii_digit()) {
            &model[..pos]
        } else {
            model
        }
    } else {
        model
    };

    let parts: Vec<&str> = base.split('-').collect();
    let mut result: Vec<String> = Vec::new();
    let mut i = 0;

    while i < parts.len() {
        let part = parts[i];

        if part.eq_ignore_ascii_case("gpt") {
            result.push("GPT".into());
        } else if part.eq_ignore_ascii_case("claude") {
            result.push("Claude".into());
        } else if part.eq_ignore_ascii_case("latest") {
            // skip
        } else if part.len() >= 2
            && part.as_bytes()[0].eq_ignore_ascii_case(&b'o')
            && part[1..].chars().all(|c| c.is_ascii_digit())
        {
            result.push(part.to_lowercase());
        } else if part.chars().all(|c| c.is_ascii_digit()) {
            let mut version = part.to_string();
            while i + 1 < parts.len()
                && parts[i + 1].len() <= 2
                && parts[i + 1].chars().all(|c| c.is_ascii_digit())
            {
                i += 1;
                version.push('.');
                version.push_str(parts[i]);
            }
            result.push(version);
        } else if part.contains('.') && part.chars().all(|c| c.is_ascii_digit() || c == '.') {
            result.push(part.into());
        } else {
            let mut chars = part.chars();
            let formatted = match chars.next() {
                None => String::new(),
                Some(c) => {
                    format!("{}{}", c.to_uppercase().collect::<String>(), chars.as_str())
                }
            };
            result.push(formatted);
        }
        i += 1;
    }

    if result.is_empty() {
        return model.to_string();
    }
    result.join(" ")
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

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;

    use super::*;
    use crate::tui::app::{BackgroundSubagentInfo, ChatMessage};

    fn rows(terminal: &Terminal<TestBackend>) -> Vec<String> {
        let buf = terminal.backend().buffer();
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| {
                        buf.cell((x, y))
                            .map(|c| c.symbol().chars().next().unwrap_or(' '))
                            .unwrap_or(' ')
                    })
                    .collect::<String>()
            })
            .collect()
    }

    fn row(terminal: &Terminal<TestBackend>, y: u16) -> String {
        rows(terminal).get(y as usize).cloned().unwrap_or_default()
    }

    fn app() -> App {
        App::new(
            "gpt-4o".to_string(),
            "openai".to_string(),
            "dot".to_string(),
            "dark",
            true,
            crate::config::CursorShape::Block,
            true,
            None,
            None,
        )
    }

    #[test]
    fn crepus_shell_renders_core_regions() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = app();
        app.conversation_title = Some("render smoke".to_string());
        app.input = "hello crepus".to_string();
        app.cursor_pos = app.input.len();
        app.messages.push(ChatMessage {
            role: "user".to_string(),
            content: "test prompt".to_string(),
            tool_calls: Vec::new(),
            thinking: None,
            model: None,
            segments: None,
            chips: None,
        });
        app.messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: "test response".to_string(),
            tool_calls: Vec::new(),
            thinking: None,
            model: Some("claude-sonnet-4-20250514".to_string()),
            segments: None,
            chips: None,
        });

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        let screen = rows(&terminal).join("\n");
        let header = row(&terminal, 0);
        let input = row(&terminal, 21);
        assert!(screen.contains("render smoke"));
        assert!(screen.contains("test prompt"));
        assert!(screen.contains("test response"));
        let user_row = rows(&terminal)
            .iter()
            .position(|r| r.contains("test prompt"))
            .unwrap();
        let assistant_row = rows(&terminal)
            .iter()
            .position(|r| r.contains("test response"))
            .unwrap();
        assert!(
            assistant_row.saturating_sub(user_row) < 8,
            "messages should stack like chat, not fill the viewport\n{screen}"
        );
        assert!(screen.contains("hello crepus"));
        assert!(header.ends_with("GPT 4o"), "header: {header:?}\n{screen}");
        assert!(
            input.contains("› hello crepus"),
            "input: {input:?}\n{screen}"
        );
        assert_eq!(
            terminal.backend().buffer().cell((0, 0)).map(|c| c.bg),
            Some(Color::Reset)
        );
    }

    #[test]
    fn crepus_command_palette_anchors_above_input() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = app();
        app.input = "/he".to_string();
        app.cursor_pos = app.input.len();
        app.command_palette.open(&app.input);

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        let screen = rows(&terminal).join("\n");
        let popup = app.layout.command_palette.unwrap();
        assert!(screen.contains("commands"));
        assert!(screen.contains("/ help"), "{screen}");
        assert!(popup.y < app.layout.input.y);
        assert!(popup.height > 1);
    }

    #[test]
    fn crepus_modal_overlays_render_selected_actions() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = app();
        app.model_selector.open(
            vec![("openai".to_string(), vec!["gpt-4o".to_string()])],
            "openai",
            "gpt-4o",
        );
        app.pending_permission = Some(crate::tui::app::PendingPermission {
            tool_name: "shell".to_string(),
            input_summary: "cargo test".to_string(),
            selected: 0,
            responder: None,
        });

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        let screen = rows(&terminal).join("\n");
        assert!(screen.contains("permission"));
        assert!(screen.contains("Allow shell?"));
        assert!(screen.contains("› Allow"));
        assert!(app.layout.model_selector.is_some());
        assert!(app.layout.permission_popup.is_some());
    }

    #[test]
    fn crepus_shell_renders_plan_and_subagents() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = app();
        app.todos = vec![
            crate::agent::TodoItem {
                content: "inspect templates".to_string(),
                status: TodoStatus::Completed,
            },
            crate::agent::TodoItem {
                content: "wire subagent command".to_string(),
                status: TodoStatus::InProgress,
            },
        ];
        app.background_subagents.push(BackgroundSubagentInfo {
            id: "bg-1".to_string(),
            description: "parallel polish".to_string(),
            output: String::new(),
            tools_completed: 1,
            done: false,
            started: std::time::Instant::now(),
            finished_at: None,
            current_tool: Some("rg".to_string()),
            current_tool_detail: None,
            tool_history: Vec::new(),
            tokens: 0,
            cost: 0.0,
            text_lines: Vec::new(),
        });

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        let screen = rows(&terminal).join("\n");
        assert!(screen.contains("plan"));
        assert!(screen.contains("✓ inspect templates"));
        assert!(screen.contains("› wire subagent command"), "{screen}");
        assert!(screen.contains("subagents"));
        assert!(screen.contains("parallel polish"));
    }
}
