//! Fix review screen — step through fixable errors with diff preview.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use elm_lint::pipeline;

use crate::app::AppState;

pub fn render(state: &AppState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header / progress
            Constraint::Min(1),    // diff
            Constraint::Length(2), // actions
        ])
        .split(area);

    let items = &state.fix_review.items;
    let idx = state.fix_review.current_index;
    let total = items.len();

    if idx >= total {
        let done = Paragraph::new(" Fix review complete.")
            .style(Style::default().fg(Color::Green))
            .block(Block::default().title(" Fix Review ").borders(Borders::ALL));
        frame.render_widget(done, area);
        return;
    }

    let item = &items[idx];

    // Header with progress.
    let progress = format!(
        " Fix Review [{}/{}]  {} accepted, {} skipped ",
        idx + 1,
        total,
        state.fix_review.accepted_count,
        state.fix_review.skipped_count,
    );
    let header_lines = vec![
        Line::from(vec![Span::styled(
            &progress,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled(
                format!(" {}:{} ", item.file_path, item.error.span.start.line),
                Style::default().fg(Color::White),
            ),
            Span::styled(
                format!("[{}] ", item.error.rule),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(&item.error.message, Style::default().fg(Color::White)),
        ]),
    ];
    let header = Paragraph::new(header_lines).block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(header, chunks[0]);

    // Diff (pre-computed in enter_fix_review).
    let mut diff_lines = Vec::new();

    for hunk in item.diff.iter() {
        diff_lines.push(Line::from(Span::styled(
            format!(
                " @@ -{},{} +{},{} @@",
                hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count,
            ),
            Style::default().fg(Color::Cyan),
        )));

        for line in &hunk.lines {
            match line {
                pipeline::DiffLine::Context(l) => {
                    diff_lines.push(Line::from(Span::styled(
                        format!("  {l}"),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                pipeline::DiffLine::Removed(l) => {
                    diff_lines.push(Line::from(Span::styled(
                        format!("- {l}"),
                        Style::default().fg(Color::Red),
                    )));
                }
                pipeline::DiffLine::Added(l) => {
                    diff_lines.push(Line::from(Span::styled(
                        format!("+ {l}"),
                        Style::default().fg(Color::Green),
                    )));
                }
            }
        }
    }

    if diff_lines.is_empty() {
        diff_lines.push(Line::from(Span::styled(
            " (no visible diff)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let diff =
        Paragraph::new(diff_lines).block(Block::default().title(" Diff ").borders(Borders::ALL));
    frame.render_widget(diff, chunks[1]);

    // Actions.
    let actions = Paragraph::new(Line::from(vec![
        Span::styled(
            " y",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Accept  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "n",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Skip  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "a",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Accept Remaining  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "q",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Quit Review", Style::default().fg(Color::DarkGray)),
    ]));
    frame.render_widget(actions, chunks[2]);
}
