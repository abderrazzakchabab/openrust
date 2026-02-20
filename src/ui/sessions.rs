use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::app::App;
use crate::session::Session;
use crate::ui::{centered_rect, theme::get_theme};

pub fn draw_sessions(frame: &mut Frame, app: &App) {
    let theme = get_theme(&app.config.ui.theme);
    let area = centered_rect(80, 70, frame.area());

    frame.render_widget(Clear, area);

    let sessions = Session::list_all().unwrap_or_default();

    let items: Vec<ListItem> = if sessions.is_empty() {
        vec![ListItem::new(Line::from(vec![Span::styled(
            "  No saved sessions",
            Style::default().fg(theme.border),
        )]))]
    } else {
        sessions
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let updated = s.updated_at.format("%Y-%m-%d %H:%M").to_string();
                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(
                            format!("  {} ", i + 1),
                            Style::default().fg(theme.border),
                        ),
                        Span::styled(
                            s.title.clone(),
                            Style::default()
                                .fg(theme.fg)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("     ", Style::default()),
                        Span::styled(
                            format!("{} | {} messages | {} | {}", updated, s.message_count, s.provider, s.model),
                            Style::default().fg(theme.border),
                        ),
                    ]),
                ])
            })
            .collect()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.focused_border_style())
            .title(Span::styled(" Sessions ", theme.title_style())),
    );

    frame.render_widget(list, area);

    // Instructions at bottom
    let instructions = Paragraph::new(Line::from(vec![
        Span::styled("  Esc", Style::default().fg(theme.highlight)),
        Span::styled(" Close | ", Style::default().fg(theme.border)),
        Span::styled("Ctrl+N", Style::default().fg(theme.highlight)),
        Span::styled(" New Session", Style::default().fg(theme.border)),
    ]));

    if area.height > 3 {
        let bottom = ratatui::layout::Rect {
            x: area.x,
            y: area.y + area.height - 1,
            width: area.width,
            height: 1,
        };
        frame.render_widget(instructions, bottom);
    }
}
