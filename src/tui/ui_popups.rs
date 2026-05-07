use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Clear;

use crate::tui::app::{App, StatusMessage};
use crate::tui::widgets::{
    COMMANDS, LoginPopup, LoginStep, PaletteEntryKind, ThinkingLevel, WelcomeScreen,
};

const MODAL_TEMPLATE: &str = include_str!("crepus/modal.crepus");

struct CrepusLine {
    text: String,
    selected: bool,
    muted: bool,
    success: bool,
    accent: bool,
}

fn normalize_crepus_text(raw: &str) -> String {
    raw.replace(['\n', '\r'], " ")
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('{', "｛")
        .replace('}', "｝")
}

fn crepus_line(kind: &'static str, text: impl Into<String>) -> CrepusLine {
    CrepusLine {
        text: text.into(),
        selected: kind == "selected",
        muted: kind == "muted",
        success: kind == "success",
        accent: kind == "accent",
    }
}

fn plain_line(text: impl Into<String>) -> CrepusLine {
    crepus_line("", text)
}

fn muted_line(text: impl Into<String>) -> CrepusLine {
    crepus_line("muted", text)
}

fn selected_line(text: impl Into<String>) -> CrepusLine {
    crepus_line("selected", text)
}

fn success_line(text: impl Into<String>) -> CrepusLine {
    crepus_line("success", text)
}

fn accent_line(text: impl Into<String>) -> CrepusLine {
    crepus_line("accent", text)
}

fn empty_line() -> CrepusLine {
    plain_line("")
}

fn render_crepus_rows(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    title: &str,
    lines: &[CrepusLine],
    footer: Option<&str>,
) {
    frame.render_widget(Clear, area);
    let rows: Vec<crepuscularity_tui::TemplateContext> = lines
        .iter()
        .map(|line| {
            let mut ctx = crepuscularity_tui::TemplateContext::new();
            ctx.set("text", normalize_crepus_text(&line.text));
            ctx.set("selected", line.selected);
            ctx.set("muted", line.muted);
            ctx.set("success", line.success);
            ctx.set("accent", line.accent);
            ctx
        })
        .collect();
    let mut ctx = crepuscularity_tui::TemplateContext::new();
    ctx.set("title", normalize_crepus_text(title));
    ctx.set("rows", crepuscularity_tui::TemplateValue::List(rows));
    ctx.set("has_footer", footer.is_some());
    ctx.set("footer", normalize_crepus_text(footer.unwrap_or("")));
    if let Err(err) = crepuscularity_tui::render_template(MODAL_TEMPLATE, &ctx, frame, area) {
        app.status_message = Some(StatusMessage::error(format!(
            "crepus-ui render error: {err}"
        )));
    }
}

fn centered_popup(area: Rect, content_width: usize, content_height: usize) -> Rect {
    let width = (content_width as u16).min(area.width.saturating_sub(4));
    let height = (content_height as u16).min(area.height.saturating_sub(2));
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

fn popup_rect(area: Rect) -> Rect {
    let w = ((area.width as u32) * 60 / 100).clamp(40, 72) as u16;
    let w = w.min(area.width.saturating_sub(4));
    let h = (((area.height as u32) * 55 / 100).max(10)) as u16;
    let h = h.min(area.height.saturating_sub(4));
    let x = area.width.saturating_sub(w) / 2;
    let y = area.height.saturating_sub(h) / 2;
    Rect::new(x, y, w, h)
}

fn anchored_popup(area: Rect, input: Rect, width: u16, lines: usize) -> Rect {
    let w = width.min(area.width.saturating_sub(4)).max(24);
    let h = (lines as u16 + 5)
        .clamp(7, 14)
        .min(area.height.saturating_sub(4));
    let x = input.x.min(area.width.saturating_sub(w + 1));
    let y = input.y.saturating_sub(h).max(1);
    Rect::new(x, y, w, h)
}

fn scroll_start(selected: usize, visible: usize) -> usize {
    if selected >= visible {
        return selected.saturating_sub(visible) + 1;
    }
    0
}

pub fn draw_model_selector(frame: &mut Frame, app: &mut App) {
    let area = popup_rect(frame.area());
    app.layout.model_selector = Some(area);
    let sel = &app.model_selector;
    let visible = area.height.saturating_sub(4) as usize;
    let start = scroll_start(sel.selected, visible);
    let mut lines = vec![muted_line(if sel.query.is_empty() {
        "type to filter...".to_string()
    } else {
        format!("filter: {}", sel.query)
    })];
    for (item, &idx) in sel
        .filtered
        .iter()
        .enumerate()
        .skip(start)
        .take(visible.saturating_sub(1))
    {
        let entry = &app.model_selector.entries[idx];
        let current = entry.provider == sel.current_provider && entry.model == sel.current_model;
        let favorite = sel.favorites.contains(&entry.model);
        let marker = if favorite { "* " } else { "  " };
        let text = format!(
            "{}{} / {}{}",
            marker,
            entry.provider,
            crate::tui::ui::display_model(&entry.model),
            if current { " current" } else { "" }
        );
        if item == sel.selected {
            lines.push(selected_line(text));
        } else if current {
            lines.push(success_line(text));
        } else {
            lines.push(plain_line(text));
        }
    }
    if sel.filtered.is_empty() {
        lines.push(muted_line("no matches"));
    }
    render_crepus_rows(
        frame,
        app,
        area,
        "model",
        &lines,
        Some("up/down select  enter confirm  s favorite  esc cancel"),
    );
}

pub fn draw_agent_selector(frame: &mut Frame, app: &mut App) {
    let area = popup_rect(frame.area());
    app.layout.agent_selector = Some(area);
    let sel = &app.agent_selector;
    let visible = area.height.saturating_sub(4) as usize;
    let start = scroll_start(sel.selected, visible);
    let lines: Vec<CrepusLine> = sel
        .entries
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
        .map(|(i, e)| {
            let text = format!("{}  {}", e.name, e.description);
            if i == sel.selected {
                selected_line(text)
            } else if e.name == sel.current {
                success_line(text)
            } else {
                plain_line(text)
            }
        })
        .collect();
    render_crepus_rows(
        frame,
        app,
        area,
        "agent",
        &lines,
        Some("up/down select  enter confirm  esc cancel"),
    );
}

pub fn draw_command_palette(frame: &mut Frame, app: &mut App, input_area: Rect) {
    let palette = &app.command_palette;
    if palette.filtered.is_empty() {
        app.layout.command_palette = None;
        return;
    }
    let area = anchored_popup(frame.area(), input_area, 72, palette.filtered.len());
    app.layout.command_palette = Some(area);
    let visible = area.height.saturating_sub(4) as usize;
    let start = scroll_start(palette.selected, visible);
    let mut lines = vec![muted_line(format!(
        "/{}",
        app.input.trim_start_matches('/')
    ))];
    lines.extend(
        palette
            .filtered
            .iter()
            .enumerate()
            .skip(start)
            .take(visible.saturating_sub(1))
            .map(|(i, &idx)| {
                let entry = &palette.entries[idx];
                let prefix = if entry.kind == PaletteEntryKind::Skill {
                    "◇ "
                } else {
                    "/ "
                };
                let text = format!("{}{}  {}", prefix, entry.name, entry.description);
                if i == palette.selected {
                    selected_line(text)
                } else {
                    plain_line(text)
                }
            }),
    );
    render_crepus_rows(frame, app, area, "commands", &lines, None);
}

pub fn draw_thinking_selector(frame: &mut Frame, app: &mut App) {
    let area = popup_rect(frame.area());
    app.layout.thinking_selector = Some(area);
    let lines: Vec<CrepusLine> = ThinkingLevel::all()
        .iter()
        .enumerate()
        .map(|(i, level)| {
            let text = format!("{}  {}", level.label(), level.description());
            if i == app.thinking_selector.selected {
                selected_line(text)
            } else if *level == app.thinking_selector.current {
                success_line(text)
            } else {
                plain_line(text)
            }
        })
        .collect();
    render_crepus_rows(
        frame,
        app,
        area,
        "thinking",
        &lines,
        Some("up/down select  enter confirm  esc cancel"),
    );
}

pub fn draw_help_popup(frame: &mut Frame, app: &mut App) {
    let area = popup_rect(frame.area());
    app.layout.help_popup = Some(area);
    let mut lines: Vec<CrepusLine> = vec![accent_line("commands")];
    lines.extend(COMMANDS.iter().map(|c| {
        let suffix = if c.shortcut.is_empty() {
            String::new()
        } else {
            format!("  {}", c.shortcut)
        };
        plain_line(format!("/{:<12} {}{}", c.name, c.description, suffix))
    }));
    lines.push(empty_line());
    lines.push(accent_line("navigation"));
    if app.vim_mode {
        lines.extend([
            muted_line("j/k scroll up/down"),
            muted_line("g/G top/bottom"),
            muted_line("i/Esc insert/normal mode"),
        ]);
    } else {
        lines.extend([
            muted_line("Up/Down scroll messages"),
            muted_line("PgUp/PgDn page scroll"),
            muted_line("^D half-page down"),
        ]);
    }
    render_crepus_rows(frame, app, area, "help", &lines, Some("esc close"));
}

pub fn draw_session_selector(frame: &mut Frame, app: &mut App) {
    let area = popup_rect(frame.area());
    app.layout.session_selector = Some(area);
    let sel = &app.session_selector;
    let visible = area.height.saturating_sub(4) as usize;
    let start = scroll_start(sel.selected, visible);
    let mut lines = vec![muted_line(if sel.query.is_empty() {
        "type to filter...".to_string()
    } else {
        format!("filter: {}", sel.query)
    })];
    for (item, &idx) in sel
        .filtered
        .iter()
        .enumerate()
        .skip(start)
        .take(visible.saturating_sub(1))
    {
        let entry = &app.session_selector.entries[idx];
        let text = format!("{}  {}", entry.title, entry.subtitle);
        if item == sel.selected {
            lines.push(selected_line(text));
        } else {
            lines.push(plain_line(text));
        }
    }
    if sel.filtered.is_empty() && !sel.query.is_empty() {
        lines.push(muted_line("no matches"));
    } else if sel.entries.is_empty() {
        lines.push(muted_line("no sessions in this directory"));
    }
    render_crepus_rows(
        frame,
        app,
        area,
        "sessions",
        &lines,
        Some("up/down select  enter resume  esc cancel"),
    );
}

pub fn draw_context_menu(frame: &mut Frame, app: &mut App) {
    let labels = crate::tui::widgets::MessageContextMenu::labels();
    let width = labels.iter().map(|l| l.len()).max().unwrap_or(10) as u16 + 6;
    let height = labels.len() as u16 + 4;
    let area = Rect::new(
        app.context_menu
            .screen_x
            .min(frame.area().width.saturating_sub(width)),
        app.context_menu
            .screen_y
            .min(frame.area().height.saturating_sub(height)),
        width.min(frame.area().width.saturating_sub(2)),
        height.min(frame.area().height.saturating_sub(2)),
    );
    app.layout.context_menu = Some(area);
    let lines: Vec<CrepusLine> = labels
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if i == app.context_menu.selected {
                selected_line(*s)
            } else {
                plain_line(*s)
            }
        })
        .collect();
    render_crepus_rows(frame, app, area, "menu", &lines, None);
}

pub fn draw_question_popup(frame: &mut Frame, app: &mut App) {
    let Some(pq) = app.pending_question.as_ref() else {
        return;
    };
    let area = popup_rect(frame.area());
    app.layout.question_popup = Some(area);
    let mut lines = vec![plain_line(pq.question.clone()), empty_line()];
    for (i, opt) in pq.options.iter().enumerate() {
        if i == pq.selected {
            lines.push(selected_line(opt.clone()));
        } else {
            lines.push(plain_line(opt.clone()));
        }
    }
    let custom = if pq.custom_input.is_empty() {
        "type your answer...".to_string()
    } else {
        pq.custom_input.clone()
    };
    if pq.selected >= pq.options.len() {
        lines.push(selected_line(custom));
    } else {
        lines.push(muted_line(custom));
    }
    render_crepus_rows(
        frame,
        app,
        area,
        "question",
        &lines,
        Some("up/down select  enter confirm  esc cancel"),
    );
}

pub fn draw_permission_popup(frame: &mut Frame, app: &mut App) {
    let Some(p) = app.pending_permission.as_ref() else {
        return;
    };
    let area = popup_rect(frame.area());
    app.layout.permission_popup = Some(area);
    let mut lines = vec![
        accent_line(format!("Allow {}?", p.tool_name)),
        muted_line(p.input_summary.clone()),
        empty_line(),
    ];
    for (idx, label) in ["Allow", "Deny"].iter().enumerate() {
        if idx == p.selected {
            lines.push(selected_line(*label));
        } else {
            lines.push(plain_line(*label));
        }
    }
    render_crepus_rows(
        frame,
        app,
        area,
        "permission",
        &lines,
        Some("y allow  n deny  esc cancel"),
    );
}

pub fn draw_rename_popup(frame: &mut Frame, app: &mut App) {
    let area = centered_popup(frame.area(), 48, 7);
    let lines = vec![selected_line(if app.rename_input.is_empty() {
        "session title".to_string()
    } else {
        app.rename_input.clone()
    })];
    render_crepus_rows(
        frame,
        app,
        area,
        "rename",
        &lines,
        Some("enter save  esc cancel"),
    );
}

pub fn draw_file_picker(frame: &mut Frame, app: &mut App, input_area: Rect) {
    let picker = &app.file_picker;
    if picker.filtered.is_empty() {
        app.layout.file_picker = None;
        return;
    }
    let area = anchored_popup(frame.area(), input_area, 72, picker.filtered.len());
    app.layout.file_picker = Some(area);
    let visible = area.height.saturating_sub(4) as usize;
    let start = scroll_start(picker.selected, visible);
    let mut lines = vec![muted_line(if picker.query.is_empty() {
        "type to filter files...".to_string()
    } else {
        format!("filter: {}", picker.query)
    })];
    for (item, &idx) in picker
        .filtered
        .iter()
        .enumerate()
        .skip(start)
        .take(visible.saturating_sub(1))
    {
        let entry = &app.file_picker.entries[idx];
        let display = if entry.is_dir {
            format!("{}/", entry.path)
        } else {
            entry.path.clone()
        };
        if item == picker.selected {
            lines.push(selected_line(display));
        } else if entry.is_dir {
            lines.push(accent_line(display));
        } else {
            lines.push(plain_line(display));
        }
    }
    render_crepus_rows(frame, app, area, "files", &lines, None);
}

pub fn draw_login_popup(frame: &mut Frame, app: &mut App) {
    let lp = &app.login_popup;
    let (title, hint) = match lp.step {
        LoginStep::SelectProvider => ("login", "esc cancel  ↑↓ navigate  enter select"),
        LoginStep::SelectMethod => ("anthropic", "esc back  ↑↓ navigate  enter select"),
        LoginStep::EnterApiKey => ("enter api key", "esc back  enter submit"),
        LoginStep::OAuthWaiting => ("authorize", "esc cancel  enter submit code"),
        LoginStep::OAuthExchanging => ("exchanging...", "esc cancel"),
    };

    let area = popup_rect(frame.area());
    app.layout.login_popup = Some(area);
    let mut lines: Vec<CrepusLine> = vec![empty_line()];

    if lp.step == LoginStep::OAuthWaiting {
        lines.push(accent_line("browser opened for authorization"));
        lines.push(empty_line());
        lines.push(plain_line("paste the URL or code after authorizing:"));
        lines.push(empty_line());
        let display = if lp.code_input.is_empty() {
            "paste code here...".to_string()
        } else if lp.code_input.len() > 40 {
            format!("{}...", &lp.code_input[..37])
        } else {
            lp.code_input.clone()
        };
        lines.push(selected_line(display));
    } else if lp.step == LoginStep::OAuthExchanging {
        lines.push(accent_line("exchanging code for credentials..."));
    } else if lp.step == LoginStep::EnterApiKey {
        let provider = lp.provider.as_deref().unwrap_or("provider");
        lines.push(accent_line(format!("{} API key", provider)));
        lines.push(empty_line());
        let masked = "•".repeat(lp.key_input.len());
        lines.push(selected_line(if masked.is_empty() {
            "paste or type your key...".to_string()
        } else {
            masked
        }));
    } else {
        let items: Vec<(&str, bool)> = match lp.step {
            LoginStep::SelectProvider => LoginPopup::providers()
                .iter()
                .enumerate()
                .map(|(i, p)| (*p, i == lp.selected))
                .collect(),
            LoginStep::SelectMethod => LoginPopup::anthropic_methods()
                .iter()
                .enumerate()
                .map(|(i, m)| (*m, i == lp.selected))
                .collect(),
            _ => Vec::new(),
        };
        for (label, selected) in &items {
            if *selected {
                lines.push(selected_line(*label));
            } else {
                lines.push(plain_line(*label));
            }
        }
    }

    render_crepus_rows(frame, app, area, title, &lines, Some(hint));
}

pub fn draw_welcome_screen(frame: &mut Frame, app: &mut App) {
    frame.render_widget(Clear, frame.area());
    let choices = WelcomeScreen::choices();
    let area = centered_popup(frame.area(), 54, choices.len() * 2 + 7);
    app.layout.welcome_screen = Some(area);
    let mut lines = vec![
        accent_line("dot"),
        muted_line("minimal ai agent"),
        empty_line(),
    ];
    lines.extend(choices.iter().enumerate().flat_map(|(i, (label, desc))| {
        let first = if i == app.welcome_screen.selected {
            selected_line(*label)
        } else {
            plain_line(*label)
        };
        [first, muted_line(format!("  {}", desc))]
    }));
    render_crepus_rows(
        frame,
        app,
        area,
        "welcome",
        &lines,
        Some("up/down navigate  enter select  esc dismiss"),
    );
}

pub fn draw_aside_popup(frame: &mut Frame, app: &mut App) {
    let area = centered_popup(frame.area(), 80, 18);
    app.layout.aside_popup = Some(area);
    let mut lines = vec![accent_line(app.aside_popup.question.clone())];
    lines.push(empty_line());
    if app.aside_popup.response.is_empty() && !app.aside_popup.done {
        lines.push(muted_line("thinking..."));
    } else {
        let height = area.height.saturating_sub(5) as usize;
        let start = app.aside_popup.scroll_offset as usize;
        lines.extend(
            app.aside_popup
                .response
                .lines()
                .skip(start)
                .take(height)
                .map(|line| plain_line(line.to_string())),
        );
    }
    let hint = if app.aside_popup.done {
        "esc/space dismiss  up/down scroll"
    } else {
        "esc dismiss  streaming..."
    };
    render_crepus_rows(frame, app, area, "aside", &lines, Some(hint));
}
