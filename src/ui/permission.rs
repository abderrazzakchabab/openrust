use ratatui::{
    layout::Alignment,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::app::App;
use crate::ui::{centered_rect, theme::get_theme};

pub fn draw_permission_prompt(frame: &mut Frame, app: &App) {
    let theme = get_theme(&app.config.ui.theme);
    let area = centered_rect(60, 40, frame.area());

    frame.render_widget(Clear, area);

    let Some(pending) = &app.pending_permission else {
        return;
    };

    let mut lines = vec![
        Line::from(vec![Span::styled(
            "Permission Required",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Tool: ", Style::default().fg(theme.border)),
            Span::styled(
                &pending.tool_name,
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
    ];

    let desc_lines: Vec<&str> = pending.description.lines().take(10).collect();
    for line in desc_lines {
        lines.push(Line::from(vec![Span::styled(
            line,
            Style::default().fg(theme.fg),
        )]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "Options:",
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(vec![
        Span::styled("  [Y] ", Style::default().fg(theme.highlight)),
        Span::styled("Yes (allow this time)", Style::default().fg(theme.fg)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [A] ", Style::default().fg(theme.highlight)),
        Span::styled(
            "Yes, always allow this session",
            Style::default().fg(theme.fg),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [N] ", Style::default().fg(theme.highlight)),
        Span::styled("No (deny)", Style::default().fg(theme.fg)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  [Esc] ", Style::default().fg(theme.highlight)),
        Span::styled("Cancel (deny)", Style::default().fg(theme.fg)),
    ]));

    let prompt = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.focused_border_style())
                .title(Span::styled(" Permission Request ", theme.title_style())),
        )
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });

    frame.render_widget(prompt, area);
}

pub fn draw_question_prompt(frame: &mut Frame, app: &App) {
    let theme = get_theme(&app.config.ui.theme);
    let area = centered_rect(60, 50, frame.area());

    frame.render_widget(Clear, area);

    let Some(pending) = &app.pending_question else {
        return;
    };

    let mut lines = vec![
        Line::from(vec![Span::styled(
            "Question",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            &pending.question,
            Style::default().fg(theme.fg),
        )]),
        Line::from(""),
    ];

    if !pending.options.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "Select an option:",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(""));

        for (idx, option) in pending.options.iter().enumerate() {
            let is_selected = idx == app.selected_option;
            let marker = if is_selected { "▶ " } else { "  " };
            let style = if is_selected {
                Style::default()
                    .fg(theme.highlight)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.fg)
            };
            lines.push(Line::from(vec![
                Span::styled(marker, style),
                Span::styled(format!("{}. {}", idx + 1, option), style),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "Up/Down to select, Enter to confirm, Esc to cancel",
            Style::default().fg(theme.border),
        )]));
    } else {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "Type your response and press Enter",
            Style::default().fg(theme.border),
        )]));
    }

    let prompt = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.focused_border_style())
                .title(Span::styled(" Question ", theme.title_style())),
        )
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });

    frame.render_widget(prompt, area);
}
