use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use serde_json::Value;

use crate::ai::types::{ContentBlock, Role};
use crate::app::App;
use crate::ui::theme::get_theme;

pub fn draw_chat(frame: &mut Frame, app: &App) {
    let _theme = get_theme(&app.config.ui.theme);
    let size = frame.area();

    let newline_count = app.input.chars().filter(|&c| c == '\n').count();
    let input_height = (3 + newline_count as u16).min(10);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(input_height),
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
                Span::styled(
                    "  OpenRust",
                    Style::default()
                        .fg(theme.title)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " - AI Terminal Coding Assistant",
                    Style::default().fg(theme.fg),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Provider: ", Style::default().fg(theme.border)),
                Span::styled(
                    &app.config.provider.default,
                    Style::default().fg(theme.accent),
                ),
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
                let has_tool_results = msg
                    .content
                    .iter()
                    .any(|b| matches!(b, ContentBlock::ToolResult { .. }));
                if has_tool_results {
                    for block in &msg.content {
                        if let ContentBlock::ToolResult {
                            content, is_error, ..
                        } = block
                        {
                            render_tool_result(content, *is_error, &mut all_lines, &theme);
                        }
                    }
                } else {
                    let content = msg.text_content();
                    all_lines.push(Line::from(vec![
                        Span::styled("  You", theme.user_style()),
                        Span::styled("  ", Style::default()),
                    ]));
                    all_lines.push(Line::from(""));
                    for line in content.lines() {
                        all_lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(line.to_string(), Style::default().fg(theme.user_msg)),
                        ]));
                    }
                }
                all_lines.push(Line::from(""));
            }
            Role::Assistant => {
                all_lines.push(Line::from(vec![
                    Span::styled(
                        "  Assistant",
                        theme.assistant_style().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("  ", Style::default()),
                ]));
                all_lines.push(Line::from(""));
                for block in &msg.content {
                    match block {
                        ContentBlock::Text { text } => {
                            render_markdown_lines(text, &mut all_lines, &theme);
                        }
                        ContentBlock::ToolUse { name, input, .. } => {
                            render_tool_use(name, input, &mut all_lines, &theme);
                        }
                        ContentBlock::ToolResult {
                            content, is_error, ..
                        } => {
                            render_tool_result(content, *is_error, &mut all_lines, &theme);
                        }
                        ContentBlock::Thinking { thinking } => {
                            render_thinking(thinking, &mut all_lines, &theme);
                        }
                    }
                }
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
            Span::styled(
                "Thinking...",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::DIM),
            ),
        ]));
    }

    let total_lines = all_lines.len();
    let visible_height = area.height.saturating_sub(2) as usize;

    let scroll = if app.scroll_offset == usize::MAX {
        total_lines.saturating_sub(visible_height)
    } else {
        app.scroll_offset
            .min(total_lines.saturating_sub(visible_height))
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

#[allow(dead_code)]
fn render_content_blocks(content: &[ContentBlock]) -> String {
    content
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text } => text.clone(),
            ContentBlock::ToolUse { name, .. } => format!("[tool_use: {}]", name),
            ContentBlock::ToolResult { tool_use_id, .. } => {
                format!("[tool_result: {}]", tool_use_id)
            }
            ContentBlock::Thinking { thinking } => thinking.clone(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_markdown_lines(content: &str, lines: &mut Vec<Line>, theme: &crate::ui::theme::Theme) {
    let parser = Parser::new(content);
    let mut current_spans: Vec<Span> = vec![Span::raw("  ")];
    let mut in_code_block = false;
    let mut code_content = String::new();
    let mut code_lang = String::new();
    let mut list_depth: usize = 0;
    let mut in_heading = false;
    let mut heading_level = 0u8;
    let mut in_table = false;
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut current_cell = String::new();
    let mut in_emphasis = false;
    let mut in_strong = false;
    let mut in_code = false;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                in_heading = true;
                heading_level = level as u8;
                current_spans.clear();
                current_spans.push(Span::raw("  "));
            }
            Event::End(TagEnd::Heading(_)) => {
                let style = match heading_level {
                    1 => Style::default()
                        .fg(theme.title)
                        .add_modifier(Modifier::BOLD),
                    2 => Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                    _ => Style::default()
                        .fg(theme.highlight)
                        .add_modifier(Modifier::BOLD),
                };
                let text: String = current_spans
                    .iter()
                    .skip(1)
                    .map(|s| s.content.to_string())
                    .collect();
                lines.push(Line::from(vec![Span::raw("  "), Span::styled(text, style)]));
                current_spans.clear();
                current_spans.push(Span::raw("  "));
                in_heading = false;
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                code_lang = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    _ => String::new(),
                };
                code_content.clear();
                let label = if code_lang.is_empty() {
                    "code".to_string()
                } else {
                    code_lang.clone()
                };
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("```{}", label), Style::default().fg(theme.border)),
                ]));
            }
            Event::End(TagEnd::CodeBlock) => {
                for line in code_content.lines() {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            line.to_string(),
                            Style::default().fg(Color::Rgb(249, 226, 175)),
                        ),
                    ]));
                }
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("```", Style::default().fg(theme.border)),
                ]));
                in_code_block = false;
                code_content.clear();
            }
            Event::Start(Tag::List(_)) => {
                list_depth += 1;
            }
            Event::End(TagEnd::List(_)) => {
                list_depth = list_depth.saturating_sub(1);
            }
            Event::Start(Tag::Item) => {
                current_spans.clear();
                let indent = "  ".repeat(list_depth + 1);
                current_spans.push(Span::raw(indent));
                current_spans.push(Span::styled("• ", Style::default().fg(theme.accent)));
            }
            Event::End(TagEnd::Item) => {
                if !current_spans.is_empty() {
                    lines.push(Line::from(current_spans.clone()));
                }
                current_spans.clear();
                current_spans.push(Span::raw("  "));
            }
            Event::Start(Tag::Paragraph) => {
                if !in_code_block && list_depth == 0 {
                    current_spans.clear();
                    current_spans.push(Span::raw("  "));
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if !in_code_block && list_depth == 0 && !in_heading {
                    if !current_spans.is_empty() {
                        lines.push(Line::from(current_spans.clone()));
                    }
                    current_spans.clear();
                    current_spans.push(Span::raw("  "));
                }
            }
            Event::Start(Tag::Table(_)) => {
                in_table = true;
                table_rows.clear();
            }
            Event::End(TagEnd::Table) => {
                if !table_rows.is_empty() {
                    render_table(&table_rows, lines, theme);
                }
                in_table = false;
                table_rows.clear();
            }
            Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) => {
                current_row.clear();
            }
            Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {
                if !current_row.is_empty() {
                    table_rows.push(current_row.clone());
                }
                current_row.clear();
            }
            Event::Start(Tag::TableCell) => {
                current_cell.clear();
            }
            Event::End(TagEnd::TableCell) => {
                current_row.push(current_cell.clone());
                current_cell.clear();
            }
            Event::Start(Tag::Emphasis) => {
                in_emphasis = true;
            }
            Event::End(TagEnd::Emphasis) => {
                in_emphasis = false;
            }
            Event::Start(Tag::Strong) => {
                in_strong = true;
            }
            Event::End(TagEnd::Strong) => {
                in_strong = false;
            }
            Event::Code(code) => {
                let style = Style::default().fg(Color::Rgb(249, 226, 175));
                if in_table {
                    current_cell.push_str(&code);
                } else {
                    current_spans.push(Span::styled(code.to_string(), style));
                }
            }
            Event::Text(text) => {
                if in_code_block {
                    code_content.push_str(&text);
                } else if in_table {
                    current_cell.push_str(&text);
                } else {
                    let mut style = Style::default().fg(theme.fg);
                    if in_strong {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    if in_emphasis {
                        style = style.add_modifier(Modifier::ITALIC);
                    }
                    current_spans.push(Span::styled(text.to_string(), style));
                }
            }
            Event::SoftBreak => {
                if !in_code_block && !in_table {
                    current_spans.push(Span::raw(" "));
                }
            }
            Event::HardBreak => {
                if !in_code_block && !in_table {
                    if !current_spans.is_empty() {
                        lines.push(Line::from(current_spans.clone()));
                    }
                    current_spans.clear();
                    current_spans.push(Span::raw("  "));
                }
            }
            _ => {}
        }
    }

    if !current_spans.is_empty() && current_spans.len() > 1 {
        lines.push(Line::from(current_spans));
    }
}

fn render_table(rows: &[Vec<String>], lines: &mut Vec<Line>, theme: &crate::ui::theme::Theme) {
    if rows.is_empty() {
        return;
    }

    let num_cols = rows[0].len();
    let mut col_widths = vec![0; num_cols];

    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < num_cols {
                col_widths[i] = col_widths[i].max(cell.len());
            }
        }
    }

    for (row_idx, row) in rows.iter().enumerate() {
        let mut row_spans = vec![Span::raw("  ")];
        row_spans.push(Span::styled("| ", Style::default().fg(theme.border)));

        for (i, cell) in row.iter().enumerate() {
            let padded = format!("{:width$}", cell, width = col_widths[i]);
            row_spans.push(Span::styled(padded, Style::default().fg(theme.fg)));
            row_spans.push(Span::styled(" | ", Style::default().fg(theme.border)));
        }
        lines.push(Line::from(row_spans));

        if row_idx == 0 {
            let mut separator_spans = vec![Span::raw("  ")];
            separator_spans.push(Span::styled("|", Style::default().fg(theme.border)));
            for &width in &col_widths {
                separator_spans.push(Span::styled(
                    "-".repeat(width + 2),
                    Style::default().fg(theme.border),
                ));
                separator_spans.push(Span::styled("|", Style::default().fg(theme.border)));
            }
            lines.push(Line::from(separator_spans));
        }
    }
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let theme = get_theme(&app.config.ui.theme);

    let provider_info = format!(" {} | {} ", app.config.provider.default, app.get_model());

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

    let input_lines = if app.input.is_empty() && !app.is_loading {
        vec![Line::from(Span::styled(
            "Type your message... (Enter to send, Shift+Enter for newline)",
            Style::default().fg(theme.border),
        ))]
    } else {
        app.input
            .split('\n')
            .map(|line| Line::from(Span::styled(line.to_string(), theme.input_style())))
            .collect::<Vec<_>>()
    };

    let input = Paragraph::new(input_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if app.is_loading {
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::DIM)
                } else {
                    theme.focused_border_style()
                })
                .title(Span::styled(" Input ", theme.title_style())),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(input, area);

    if !app.is_loading {
        let text_before_cursor = &app.input[..app.cursor_pos];
        let lines_before_cursor: Vec<&str> = text_before_cursor.split('\n').collect();
        let cursor_line = lines_before_cursor.len().saturating_sub(1);
        let cursor_col = lines_before_cursor.last().map_or(0, |s| s.len());

        let cursor_x = (area.x + 1 + cursor_col as u16).min(area.width.saturating_sub(2));
        let cursor_y = (area.y + 1 + cursor_line as u16).min(area.height.saturating_sub(2));
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

fn render_tool_use(
    name: &str,
    input: &Value,
    lines: &mut Vec<Line>,
    theme: &crate::ui::theme::Theme,
) {
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("⚡ ", Style::default().fg(theme.accent)),
        Span::styled("Tool: ", Style::default().fg(theme.border)),
        Span::styled(
            name.to_string(),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    if let Ok(formatted) = serde_json::to_string_pretty(input) {
        for param_line in formatted.lines() {
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    param_line.to_string(),
                    Style::default()
                        .fg(theme.border)
                        .add_modifier(Modifier::DIM),
                ),
            ]));
        }
    }
}

fn render_tool_result(
    content: &str,
    is_error: bool,
    lines: &mut Vec<Line>,
    theme: &crate::ui::theme::Theme,
) {
    let (icon, header_style) = if is_error {
        ("✗ ", Style::default().fg(Color::Rgb(243, 139, 168)))
    } else {
        ("✓ ", Style::default().fg(Color::Rgb(166, 227, 161)))
    };
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(icon, header_style),
        Span::styled(if is_error { "Error" } else { "Result" }, header_style),
    ]));
    let max_display = 50;
    let content_lines: Vec<&str> = content.lines().collect();
    let truncated = content_lines.len() > max_display;
    let show = if truncated {
        &content_lines[..max_display]
    } else {
        &content_lines[..]
    };
    let style = if is_error {
        Style::default()
            .fg(Color::Rgb(243, 139, 168))
            .add_modifier(Modifier::DIM)
    } else {
        Style::default().fg(theme.fg).add_modifier(Modifier::DIM)
    };
    for line in show {
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(line.to_string(), style),
        ]));
    }
    if truncated {
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(
                format!("... ({} more lines)", content_lines.len() - max_display),
                Style::default()
                    .fg(theme.border)
                    .add_modifier(Modifier::DIM),
            ),
        ]));
    }
}

fn render_thinking(thinking: &str, lines: &mut Vec<Line>, theme: &crate::ui::theme::Theme) {
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            "Thinking...",
            Style::default()
                .fg(theme.border)
                .add_modifier(Modifier::DIM | Modifier::ITALIC),
        ),
    ]));
    for line in thinking.lines().take(20) {
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(
                line.to_string(),
                Style::default()
                    .fg(theme.border)
                    .add_modifier(Modifier::DIM),
            ),
        ]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::Theme;
    use ratatui::style::Color;

    fn test_theme() -> Theme {
        Theme {
            bg: Color::Rgb(30, 30, 46),
            fg: Color::Rgb(205, 214, 244),
            border: Color::Rgb(108, 112, 134),
            accent: Color::Rgb(137, 180, 250),
            user_msg: Color::Rgb(166, 227, 161),
            assistant_msg: Color::Rgb(245, 224, 220),
            system_msg: Color::Rgb(243, 139, 168),
            input_bg: Color::Rgb(24, 24, 37),
            input_fg: Color::Rgb(205, 214, 244),
            status_ok: Color::Rgb(166, 227, 161),
            status_err: Color::Rgb(243, 139, 168),
            highlight: Color::Rgb(249, 226, 175),
            title: Color::Rgb(180, 190, 254),
        }
    }

    #[test]
    fn test_markdown_code_block() {
        let markdown = "```rust\nfn main() {\n    println!(\"Hello\");\n}\n```";
        let mut lines = Vec::new();
        render_markdown_lines(markdown, &mut lines, &test_theme());

        assert!(!lines.is_empty());
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("```rust"));
        assert!(text.contains("fn main()"));
        assert!(text.contains("println!"));
    }

    #[test]
    fn test_markdown_heading() {
        let markdown = "# Heading 1\n## Heading 2\n### Heading 3";
        let mut lines = Vec::new();
        render_markdown_lines(markdown, &mut lines, &test_theme());

        assert!(lines.len() >= 3);
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Heading 1"));
        assert!(text.contains("Heading 2"));
        assert!(text.contains("Heading 3"));
    }

    #[test]
    fn test_markdown_table() {
        let markdown = "| Header1 | Header2 |\n|---------|----------|\n| Cell1   | Cell2   |";
        let mut lines = Vec::new();
        render_markdown_lines(markdown, &mut lines, &test_theme());

        assert!(!lines.is_empty());
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Header1"));
        assert!(text.contains("Header2"));
        assert!(text.contains("Cell1"));
        assert!(text.contains("Cell2"));

        let separator_count = lines
            .iter()
            .filter(|l| l.to_string().contains("---"))
            .count();
        assert!(separator_count >= 1);
    }

    #[test]
    fn test_markdown_inline_code() {
        let markdown = "This is `inline code` in text.";
        let mut lines = Vec::new();
        render_markdown_lines(markdown, &mut lines, &test_theme());

        assert!(!lines.is_empty());
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("inline code"));
    }

    #[test]
    fn test_markdown_emphasis() {
        let markdown = "This is *italic* and **bold** text.";
        let mut lines = Vec::new();
        render_markdown_lines(markdown, &mut lines, &test_theme());

        assert!(!lines.is_empty());
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("italic"));
        assert!(text.contains("bold"));
    }

    #[test]
    fn test_markdown_nested_list() {
        let markdown = "- Item 1\n- Nested 1.1\n- Nested 1.2\n- Item 2";
        let mut lines = Vec::new();
        render_markdown_lines(markdown, &mut lines, &test_theme());

        assert!(!lines.is_empty());
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Item 1"));
        assert!(text.contains("Nested 1.1"));
        assert!(text.contains("Nested 1.2"));
        assert!(text.contains("Item 2"));

        let bullet_count = lines.iter().filter(|l| l.to_string().contains('•')).count();
        assert!(bullet_count >= 4);
    }

    #[test]
    fn test_table_rendering() {
        let rows = vec![
            vec!["Header1".to_string(), "Header2".to_string()],
            vec!["Cell1".to_string(), "Cell2".to_string()],
        ];
        let mut lines = Vec::new();
        render_table(&rows, &mut lines, &test_theme());

        assert!(!lines.is_empty());
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Header1"));
        assert!(text.contains("Header2"));
        assert!(text.contains("Cell1"));
        assert!(text.contains("Cell2"));
    }
}
