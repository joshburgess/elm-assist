//! Dead code browser screen.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table};

use super::helpers;
use crate::app::{AppState, TableHitTest};

pub fn render(state: &AppState, frame: &mut Frame, area: Rect) {
    if let Some(ref preview) = state.unused.preview {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);
        render_table(state, frame, chunks[0]);
        helpers::render_source_preview(preview, frame, chunks[1]);
    } else {
        render_table(state, frame, area);
    }
}

fn render_table(state: &AppState, frame: &mut Frame, area: Rect) {
    let title = format!(" Unused Code ({} findings) ", state.unused.findings.len());
    let block = Block::default().title(title).borders(Borders::ALL);

    if state.unused.findings.is_empty() {
        let msg = Paragraph::new(" No unused code found.")
            .style(Style::default().fg(Color::Green))
            .block(block);
        frame.render_widget(msg, area);
        return;
    }

    let header = Row::new(vec!["File", "Line", "Rule", "Message"]).style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let visible_height = area.height.saturating_sub(4) as usize;
    let selected = state.unused.selected_index;
    let scroll = helpers::scroll_offset(selected, visible_height);

    state.table_hit.set(TableHitTest {
        data_top: area.y.saturating_add(3),
        visible_rows: visible_height as u16,
        scroll,
    });

    let rows: Vec<Row> = state
        .unused
        .findings
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible_height)
        .map(|(i, finding)| {
            let short_path = finding
                .file_path
                .strip_prefix("src/")
                .unwrap_or(&finding.file_path);

            let style = if i == selected {
                helpers::selected_style()
            } else {
                Style::default()
            };

            Row::new(vec![
                short_path.to_string(),
                format!("{}", finding.line),
                finding.kind.to_string(),
                finding.name.clone(),
            ])
            .style(style)
            .height(1)
        })
        .collect();

    let widths = [
        Constraint::Percentage(30),
        Constraint::Length(5),
        Constraint::Percentage(25),
        Constraint::Percentage(40),
    ];

    let table = Table::new(rows, widths).header(header).block(block);
    frame.render_widget(table, area);
}
