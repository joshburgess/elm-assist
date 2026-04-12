//! Shared view helper functions.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::SourcePreview;

/// Compute scroll offset to keep the selected index visible in a list.
pub fn scroll_offset(selected: usize, visible_height: usize) -> usize {
    if selected >= visible_height {
        selected - visible_height + 1
    } else {
        0
    }
}

/// Style for the currently selected row in a table.
pub fn selected_style() -> Style {
    Style::default()
        .bg(Color::DarkGray)
        .add_modifier(Modifier::BOLD)
}

/// Render a source-code snippet around `preview.line` into `area`.
/// Shared by the Unused and Search screens.
pub fn render_source_preview(preview: &SourcePreview, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(format!(
            " {}:{} (Enter to close) ",
            preview.file_path, preview.line
        ))
        .borders(Borders::ALL);

    let source_lines: Vec<&str> = preview.source.lines().collect();
    let target = preview.line.saturating_sub(1);
    let ctx: usize = 5;
    let start = target.saturating_sub(ctx);
    let end = (target + ctx + 1).min(source_lines.len());

    let lines: Vec<Line> = source_lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let line_num = start + i + 1;
            let is_target = line_num == preview.line;
            let num_style = if is_target {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let code_style = if is_target {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };
            Line::from(vec![
                Span::styled(format!("{line_num:>4} "), num_style),
                Span::styled((*line).to_string(), code_style),
            ])
        })
        .collect();

    let content = Paragraph::new(lines).block(block);
    frame.render_widget(content, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_offset_zero_when_selected_fits() {
        assert_eq!(scroll_offset(0, 20), 0);
        assert_eq!(scroll_offset(19, 20), 0);
    }

    #[test]
    fn scroll_offset_advances_when_selected_exceeds_height() {
        assert_eq!(scroll_offset(20, 20), 1);
        assert_eq!(scroll_offset(25, 20), 6);
    }

    #[test]
    fn scroll_offset_handles_zero_height() {
        // visible_height=0 means selected >= 0 is always true
        assert_eq!(scroll_offset(0, 0), 1);
        assert_eq!(scroll_offset(5, 0), 6);
    }
}
