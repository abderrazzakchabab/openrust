pub mod chat;
pub mod help;
pub mod sessions;
pub mod theme;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};

use crate::app::{App, AppMode};
use chat::draw_chat;
use help::draw_help;
use sessions::draw_sessions;

pub fn draw(frame: &mut Frame, app: &App) {
    match app.mode {
        AppMode::Chat => draw_chat(frame, app),
        AppMode::SessionList => draw_sessions(frame, app),
        AppMode::Help => draw_help(frame, app),
        AppMode::Quit => {}
    }
}

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
