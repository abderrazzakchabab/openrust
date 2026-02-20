use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::ai::types::Role;
use crate::app::App;
use crate::ui::theme::get_theme;

pub fn draw_chat(frame: &mut Frame, app: &App) {
    let theme = get_theme(&app.config.ui.theme);
    let size = frame.area();

    // Main layout: messages + status bar + input
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // messages
            Constraint::Length(1), // status bar
            Constraint::Length(3), // input
        ])
        .split(size);

    draw_messages(frame, app, chunks[0]);
    draw_status_bar(frame, app, chunks[1]);
    draw_input(frame, app, chunks[2]);
}

fn draw_messages(frame: &mut Frame, app: &App, area: Rect) {
    let theme = get_theme(&app.config.ui.theme);

    if app.session.messages.is_empty() {
        let welcome = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  OpenRust", Style::default().fg(theme.title).add_modifier(Modifier::BOLD)),
                Span::styled(" - AI Terminal Coding Assistant", Style::default().fg(theme.fg)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Provider: ", Style::default().fg(theme.border)),
                Span::styled(&app.config.provider.default, Style::default().fg(theme.accent)),
                Span::styled("  |  Model: ", Style::default().fg(theme.border)),
                Span::styled(app.get_model(), Style::default().fg(theme.accent)),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "  Start chatting by typing your question below.",
                Style::default().fg(theme.border),
            )]),
            Line::from(vec![Span::styled(
                "  Press F1 or Ctrl+? for help.",
                Style::default().fg(theme.border),
            )]),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border_style())
                .title(Span::styled(" OpenRust ", theme.title_style())),
        );
        frame.render_widget(welcome, area);
        return;
    }

    // Build lines from messages
    let mut all_lines: Vec<Line> = Vec::new();

    for msg in &app.session.messages {
        match msg.role {
            Role::User => {
                all_lines.push(Line::from(vec![
                    Span::styled("  You", theme.user_style()),
                    Span::styled("  ", Style::default()),
                ]));
                all_lines.push(Line::from(""));
                for line in msg.content.lines() {
                    all_lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(line.to_string(), Style::default().fg(theme.user_msg)),
                    ]));
                }
                all_lines.push(Line::from(""));
            }
            Role::Assistant => {
                all_lines.push(Line::from(vec![
                    Span::styled("  Assistant", theme.assistant_style().add_modifier(Modifier::BOLD)),
                    Span::styled("  ", Style::default()),
                ]));
                all_lines.push(Line::from(""));
                render_markdown_lines(&msg.content, &mut all_lines, &theme);
                all_lines.push(Line::from(""));
            }
            Role::System => {}
        }

        // Separator
        all_lines.push(Line::from(vec![Span::styled(
            "  ",
            Style::default().fg(theme.border),
        )]));
    }

    // Loading indicator
    if app.is_loading {
        all_lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("Thinking...", Style::default().fg(theme.accent).add_modifier(Modifier::DIM)),
        ]));
    }

    let total_lines = all_lines.len();
    let visible_height = area.height.saturating_sub(2) as usize;

    let scroll = if app.scroll_offset == usize::MAX {
        total_lines.saturating_sub(visible_height)
    } else {
        app.scroll_offset.min(total_lines.saturating_sub(visible_height))
    };

    let text = Text::from(all_lines);
    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border_style())
                .title(Span::styled(
                    format!(" {} ", app.session.title),
                    theme.title_style(),
                )),
        )
        .scroll((scroll as u16, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn render_markdown_lines(content: &str, lines: &mut Vec<Line>, theme: &crate::ui::theme::Theme) {
    let mut in_code_block = false;
    let mut code_lang = String::new();

    for line in content.lines() {
        if line.starts_with("```") {
            if in_code_block {
                in_code_block = false;
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("```", Style::default().fg(theme.border)),
                ]));
            } else {
                in_code_block = true;
                code_lang = line.trim_start_matches('`').to_string();
                let label = if code_lang.is_empty() {
                    "code".to_string()
                } else {
                    code_lang.clone()
                };
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!("```{}", label),
                        Style::default().fg(theme.border),
                    ),
                ]));
            }
        } else if in_code_block {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::Rgb(249, 226, 175)),
                ),
            ]));
        } else if line.starts_with("# ") {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    line[2..].to_string(),
                    Style::default()
                        .fg(theme.title)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        } else if line.starts_with("## ") {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    line[3..].to_string(),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        } else if line.starts_with("### ") {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    line[4..].to_string(),
                    Style::default()
                        .fg(theme.highlight)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        } else if line.starts_with("- ") || line.starts_with("* ") {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("• ", Style::default().fg(theme.accent)),
                Span::styled(line[2..].to_string(), Style::default().fg(theme.fg)),
            ]));
        } else if line.is_empty() {
            lines.push(Line::from(""));
        } else {
            // Inline code in regular text
            let mut spans: Vec<Span> = vec![Span::raw("  ")];
            let parts: Vec<&str> = line.split('`').collect();
            for (i, part) in parts.iter().enumerate() {
                if i % 2 == 0 {
                    spans.push(Span::styled(part.to_string(), Style::default().fg(theme.fg)));
                } else {
                    spans.push(Span::styled(
                        part.to_string(),
                        Style::default().fg(Color::Rgb(249, 226, 175)),
                    ));
                }
            }
            lines.push(Line::from(spans));
        }
    }
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let theme = get_theme(&app.config.ui.theme);

    let provider_info = format!(
        " {} | {} ",
        app.config.provider.default,
        app.get_model()
    );

    let status = if let Some(msg) = &app.status_message {
        let is_error = msg.starts_with("Error");
        Paragraph::new(Line::from(vec![
            Span::styled(&provider_info, Style::default().fg(theme.border)),
            Span::styled(msg.clone(), theme.status_style(is_error)),
        ]))
    } else {
        let msg_count = app.session.messages.len();
        Paragraph::new(Line::from(vec![
            Span::styled(&provider_info, Style::default().fg(theme.border)),
            Span::styled(
                format!("{} messages | F1 Help | Ctrl+Q Quit", msg_count),
                Style::default().fg(theme.border),
            ),
        ]))
    };

    frame.render_widget(status, area);
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let theme = get_theme(&app.config.ui.theme);

    let input_text = if app.input.is_empty() && !app.is_loading {
        Span::styled(
            "Type your message... (Enter to send, Shift+Enter for newline)",
            Style::default().fg(theme.border),
        )
    } else {
        Span::styled(app.input.clone(), theme.input_style())
    };

    let input = Paragraph::new(Line::from(input_text))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if app.is_loading {
                    Style::default().fg(theme.accent).add_modifier(Modifier::DIM)
                } else {
                    theme.focused_border_style()
                })
                .title(Span::styled(" Input ", theme.title_style())),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(input, area);

    // Position cursor
    if !app.is_loading {
        let cursor_x = area.x + 1 + (app.cursor_pos as u16).min(area.width.saturating_sub(3));
        let cursor_y = area.y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}
