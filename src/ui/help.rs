use ratatui::{
    layout::Alignment,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::app::App;
use crate::ui::{centered_rect, theme::get_theme};

pub fn draw_help(frame: &mut Frame, app: &App) {
    let theme = get_theme(&app.config.ui.theme);
    let area = centered_rect(70, 80, frame.area());

    frame.render_widget(Clear, area);

    let lines = vec![
        Line::from(vec![Span::styled(
            "OpenRust - Keyboard Shortcuts",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Direct Shortcuts",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  Enter        ", Style::default().fg(theme.highlight)),
            Span::styled("Send message", Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+N       ", Style::default().fg(theme.highlight)),
            Span::styled("New session", Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+S       ", Style::default().fg(theme.highlight)),
            Span::styled("Save session", Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+L       ", Style::default().fg(theme.highlight)),
            Span::styled("View session list", Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+Q       ", Style::default().fg(theme.highlight)),
            Span::styled("Quit OpenRust", Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("  F1 / ?       ", Style::default().fg(theme.highlight)),
            Span::styled("Toggle this help", Style::default().fg(theme.fg)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Leader Key (Ctrl+X, then...)",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  n            ", Style::default().fg(theme.highlight)),
            Span::styled("New session", Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("  s            ", Style::default().fg(theme.highlight)),
            Span::styled("Save session", Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("  h            ", Style::default().fg(theme.highlight)),
            Span::styled("Show help", Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("  l            ", Style::default().fg(theme.highlight)),
            Span::styled("Show session list", Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("  q            ", Style::default().fg(theme.highlight)),
            Span::styled("Quit", Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("  m            ", Style::default().fg(theme.highlight)),
            Span::styled("Switch model", Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("  t            ", Style::default().fg(theme.highlight)),
            Span::styled("Switch theme", Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("  d            ", Style::default().fg(theme.highlight)),
            Span::styled("Show details", Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("  e            ", Style::default().fg(theme.highlight)),
            Span::styled("Export conversation", Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("  c            ", Style::default().fg(theme.highlight)),
            Span::styled("Compact history", Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("  i            ", Style::default().fg(theme.highlight)),
            Span::styled("Initialize project", Style::default().fg(theme.fg)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Navigation",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  Up/Down      ", Style::default().fg(theme.highlight)),
            Span::styled("Scroll messages", Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("  PgUp/PgDn    ", Style::default().fg(theme.highlight)),
            Span::styled("Scroll page", Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+End     ", Style::default().fg(theme.highlight)),
            Span::styled("Scroll to bottom", Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("  Left/Right   ", Style::default().fg(theme.highlight)),
            Span::styled("Move input cursor", Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled("  Home/End     ", Style::default().fg(theme.highlight)),
            Span::styled("Jump cursor to start/end", Style::default().fg(theme.fg)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "General",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  Esc          ", Style::default().fg(theme.highlight)),
            Span::styled("Close popup / cancel", Style::default().fg(theme.fg)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Press Esc or F1 to close",
            Style::default().fg(theme.border),
        )]),
    ];

    let help = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.focused_border_style())
                .title(Span::styled(" Help ", theme.title_style())),
        )
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });

    frame.render_widget(help, area);
}
