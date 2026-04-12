//! Search screen — semantic AST-aware code search with results.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table};

use super::helpers;
use crate::app::{AppState, InputMode, TableHitTest};

pub fn render(state: &AppState, frame: &mut Frame, area: Rect) {
    if let Some(ref preview) = state.search.preview {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // query input + help
                Constraint::Min(1),    // results + preview
            ])
            .split(area);
        render_query_input(state, frame, chunks[0]);
        let inner = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1]);
        render_results(state, frame, inner[0]);
        helpers::render_source_preview(preview, frame, inner[1]);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // query input + help
                Constraint::Min(1),    // results
            ])
            .split(area);
        render_query_input(state, frame, chunks[0]);
        render_results(state, frame, chunks[1]);
    }
}

fn render_query_input(state: &AppState, frame: &mut Frame, area: Rect) {
    let cursor = if state.input_mode == InputMode::SearchQuery {
        "|"
    } else {
        ""
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(
                " Query: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}{cursor}", state.search.query),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Query types: ", Style::default().fg(Color::DarkGray)),
            Span::styled("returns ", Style::default().fg(Color::Yellow)),
            Span::styled("type ", Style::default().fg(Color::Yellow)),
            Span::styled("case-on ", Style::default().fg(Color::Yellow)),
            Span::styled("calls ", Style::default().fg(Color::Yellow)),
            Span::styled("uses ", Style::default().fg(Color::Yellow)),
            Span::styled("def ", Style::default().fg(Color::Yellow)),
            Span::styled("unused-args ", Style::default().fg(Color::Yellow)),
            Span::styled("lambda ", Style::default().fg(Color::Yellow)),
            Span::styled("update ", Style::default().fg(Color::Yellow)),
            Span::styled("expr ", Style::default().fg(Color::Yellow)),
        ]),
    ];

    let title = if state.input_mode == InputMode::SearchQuery {
        " Search [typing] "
    } else {
        " Search (/ to type query, Enter to search) "
    };

    let input = Paragraph::new(lines).block(Block::default().title(title).borders(Borders::ALL));
    frame.render_widget(input, area);
}

fn render_results(state: &AppState, frame: &mut Frame, area: Rect) {
    let title = format!(" Results ({}) ", state.search.results.len());
    let block = Block::default().title(title).borders(Borders::ALL);

    if state.search.results.is_empty() {
        let msg = if state.search.query.is_empty() {
            " Enter a search query to find code patterns."
        } else {
            " No results found."
        };
        let empty = Paragraph::new(msg)
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(empty, area);
        return;
    }

    let header = Row::new(vec!["File", "Line", "Match"]).style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let visible_height = area.height.saturating_sub(4) as usize;
    let selected = state.search.selected_index;
    let scroll = helpers::scroll_offset(selected, visible_height);

    state.table_hit.set(TableHitTest {
        data_top: area.y.saturating_add(3),
        visible_rows: visible_height as u16,
        scroll,
    });

    let rows: Vec<Row> = state
        .search
        .results
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible_height)
        .map(|(i, result)| {
            let short_path = result
                .file_path
                .strip_prefix("src/")
                .unwrap_or(&result.file_path);

            let style = if i == selected {
                helpers::selected_style()
            } else {
                Style::default()
            };

            Row::new(vec![
                short_path.to_string(),
                format!("{}", result.line),
                result.context.clone(),
            ])
            .style(style)
            .height(1)
        })
        .collect();

    let widths = [
        Constraint::Percentage(30),
        Constraint::Length(6),
        Constraint::Percentage(60),
    ];

    let table = Table::new(rows, widths).header(header).block(block);

    frame.render_widget(table, area);
}
