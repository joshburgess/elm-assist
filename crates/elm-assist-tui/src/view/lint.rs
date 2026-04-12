//! Lint browser screen — filterable table of diagnostics with source preview.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table};

use elm_lint::rule::Severity;

use super::helpers;
use crate::app::{AppState, TableHitTest};

pub fn render(state: &AppState, frame: &mut Frame, area: Rect) {
    if !state.lint.show_preview {
        render_error_table(state, frame, area);
        return;
    }

    // Adaptive split: give more to table on short terminals.
    let table_pct = if area.height < 30 { 65 } else { 55 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(table_pct),
            Constraint::Percentage(100 - table_pct),
        ])
        .split(area);

    render_error_table(state, frame, chunks[0]);
    render_source_preview(state, frame, chunks[1]);
}

fn render_error_table(state: &AppState, frame: &mut Frame, area: Rect) {
    use crate::app::InputMode;
    let title = match state.input_mode {
        InputMode::LintFilter => format!(" Lint [/{}|] ", state.lint.filter_text),
        _ if !state.lint.filter_text.is_empty() => {
            format!(
                " Lint ({} of {} matching \"{}\") ",
                state.lint.visible_len(),
                state.lint.base_errors.len(),
                state.lint.filter_text,
            )
        }
        _ => format!(" Lint ({} findings) ", state.lint.visible_len()),
    };

    let header = Row::new(vec!["File", "Line", "Rule", "Sev", "Message"]).style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    // Table overhead: top border (1) + header (1) + header separator (1) + bottom border (1) = 4.
    let visible_height = area.height.saturating_sub(4) as usize;
    let selected = state.lint.selected_index;
    let scroll = helpers::scroll_offset(selected, visible_height);

    // Record table geometry for the mouse handler.
    state.table_hit.set(TableHitTest {
        data_top: area.y.saturating_add(3), // top border + header + separator
        visible_rows: visible_height as u16,
        scroll,
    });

    let rows: Vec<Row> = state
        .lint
        .visible_iter()
        .enumerate()
        .skip(scroll)
        .take(visible_height)
        .map(|(i, (path, err))| {
            let short_path = path.strip_prefix("src/").unwrap_or(path);
            let sev = match err.severity {
                Severity::Error => "ERR",
                Severity::Warning => "WRN",
            };
            let fixable = if err.fix.is_some() { " *" } else { "" };

            let style = if i == selected {
                helpers::selected_style()
            } else {
                match err.severity {
                    Severity::Error => Style::default().fg(Color::Red),
                    Severity::Warning => Style::default().fg(Color::Yellow),
                }
            };

            Row::new(vec![
                short_path.to_string(),
                format!("{}", err.span.start.line),
                format!("{}{fixable}", err.rule),
                sev.to_string(),
                err.message.clone(),
            ])
            .style(style)
            .height(1)
        })
        .collect();

    let widths = [
        Constraint::Percentage(25),
        Constraint::Length(5),
        Constraint::Percentage(25),
        Constraint::Length(4),
        Constraint::Percentage(40),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().title(title).borders(Borders::ALL));

    frame.render_widget(table, area);
}

fn render_source_preview(state: &AppState, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" Source Preview ")
        .borders(Borders::ALL);

    let selected = state
        .lint
        .selected_index
        .min(state.lint.visible_len().saturating_sub(1));

    let Some((path, err)) = state.lint.visible_at(selected) else {
        let msg = Paragraph::new("No findings to display.")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(msg, area);
        return;
    };

    // Try to read the source from the lint result.
    let source = state.lint.result.as_ref().and_then(|r| r.sources.get(path));

    let lines: Vec<Line> = if let Some(source) = source {
        let source_lines: Vec<&str> = source.lines().collect();
        let err_line = (err.span.start.line as usize).saturating_sub(1); // 0-indexed
        let ctx: usize = 5;
        let start = err_line.saturating_sub(ctx);
        let end = (err_line + ctx + 1).min(source_lines.len());

        source_lines[start..end]
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let line_num = start + i + 1; // 1-indexed
                let is_error_line = line_num == err.span.start.line as usize;
                let num_style = if is_error_line {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                let code_style = if is_error_line {
                    Style::default().fg(Color::White)
                } else {
                    Style::default().fg(Color::Gray)
                };

                Line::from(vec![
                    Span::styled(format!("{line_num:>4} "), num_style),
                    Span::styled((*line).to_string(), code_style),
                ])
            })
            .collect()
    } else {
        vec![Line::from(Span::styled(
            format!("Could not load source for {path}"),
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let header = Line::from(vec![
        Span::styled(
            format!(" {}:{} ", path, err.span.start.line),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled(
            format!("[{}] ", err.rule),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(&err.message, Style::default().fg(Color::White)),
    ]);

    let mut all_lines = vec![header, Line::from("")];
    all_lines.extend(lines);

    let preview = Paragraph::new(all_lines).block(block);
    frame.render_widget(preview, area);
}
