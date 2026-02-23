use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub dim: Color,
    pub accent: Color,
    pub user_label: Style,
    pub assistant_label: Style,
    pub border: Style,
    pub input_prompt: Style,
    pub status_bar: Style,
    pub code_bg: Color,
    pub inline_code: Style,
    pub error: Style,
    pub tool_name: Style,
    pub tool_output: Style,
    pub heading: Style,
    pub bold: Style,
    pub italic: Style,
    pub blockquote: Style,
    pub link: Style,
    pub list_bullet: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Reset,
            fg: Color::White,
            dim: Color::DarkGray,
            accent: Color::Cyan,
            user_label: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            assistant_label: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            border: Style::default().fg(Color::DarkGray),
            input_prompt: Style::default().fg(Color::Cyan),
            status_bar: Style::default().fg(Color::DarkGray),
            code_bg: Color::Rgb(30, 30, 46),
            inline_code: Style::default().fg(Color::Yellow),
            error: Style::default().fg(Color::Red),
            tool_name: Style::default().fg(Color::Yellow),
            tool_output: Style::default().fg(Color::DarkGray),
            heading: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            bold: Style::default().add_modifier(Modifier::BOLD),
            italic: Style::default().add_modifier(Modifier::ITALIC),
            blockquote: Style::default().fg(Color::DarkGray),
            link: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::UNDERLINED),
            list_bullet: Style::default().fg(Color::DarkGray),
        }
    }
}
