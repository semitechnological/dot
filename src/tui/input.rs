use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::app::{App, AppMode};

pub enum InputAction {
    None,
    SendMessage(String),
    Quit,
    ScrollUp(u16),
    ScrollDown(u16),
    ScrollToTop,
    ScrollToBottom,
    ClearConversation,
}

pub fn handle_key(app: &mut App, key: KeyEvent) -> InputAction {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return InputAction::Quit;
    }

    match app.mode {
        AppMode::Normal => handle_normal(app, key),
        AppMode::Insert => handle_insert(app, key),
    }
}

fn handle_normal(app: &mut App, key: KeyEvent) -> InputAction {
    match key.code {
        KeyCode::Char('q') => InputAction::Quit,
        KeyCode::Char('i') | KeyCode::Enter => {
            app.mode = AppMode::Insert;
            InputAction::None
        }
        KeyCode::Char('j') | KeyCode::Down => InputAction::ScrollDown(1),
        KeyCode::Char('k') | KeyCode::Up => InputAction::ScrollUp(1),
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            InputAction::ScrollDown(10)
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            InputAction::ScrollUp(10)
        }
        KeyCode::Char('g') => InputAction::ScrollToTop,
        KeyCode::Char('G') => InputAction::ScrollToBottom,
        KeyCode::PageUp => InputAction::ScrollUp(20),
        KeyCode::PageDown => InputAction::ScrollDown(20),
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            InputAction::ClearConversation
        }
        _ => InputAction::None,
    }
}

fn handle_insert(app: &mut App, key: KeyEvent) -> InputAction {
    if app.is_streaming {
        match key.code {
            KeyCode::Esc => {
                app.mode = AppMode::Normal;
            }
            _ => {}
        }
        return InputAction::None;
    }

    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
            InputAction::None
        }
        KeyCode::Enter => {
            if let Some(msg) = app.take_input() {
                InputAction::SendMessage(msg)
            } else {
                InputAction::None
            }
        }
        KeyCode::Char(c) => {
            app.insert_char(c);
            InputAction::None
        }
        KeyCode::Backspace => {
            app.delete_char_before();
            InputAction::None
        }
        KeyCode::Left => {
            app.move_cursor_left();
            InputAction::None
        }
        KeyCode::Right => {
            app.move_cursor_right();
            InputAction::None
        }
        KeyCode::Home => {
            app.move_cursor_home();
            InputAction::None
        }
        KeyCode::End => {
            app.move_cursor_end();
            InputAction::None
        }
        _ => InputAction::None,
    }
}
