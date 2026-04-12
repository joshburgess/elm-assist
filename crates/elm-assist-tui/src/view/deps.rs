//! Dependencies screen — stats, tree, and cycle views.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use super::helpers;
use crate::app::{AppState, DepsSubView, TableHitTest};

pub fn render(state: &AppState, frame: &mut Frame, area: Rect) {
    let title = match state.deps.sub_view {
        DepsSubView::Tree => " Dependencies [Tree] ",
        DepsSubView::Stats => " Dependencies [Stats] ",
        DepsSubView::Cycles => " Dependencies [Cycles] ",
    };

    let block = Block::default().title(title).borders(Borders::ALL);

    let Some(ref stats) = state.deps.stats else {
        let msg = Paragraph::new("Analyzing dependencies...")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(msg, area);
        return;
    };

    let lines: Vec<Line> = match state.deps.sub_view {
        DepsSubView::Stats => {
            let mut lines = vec![
                Line::from(vec![
                    Span::styled(" Modules: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{}", stats.total_modules),
                        Style::default().fg(Color::White),
                    ),
                ]),
                Line::from(vec![
                    Span::styled(" Edges:   ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{}", stats.total_edges),
                        Style::default().fg(Color::White),
                    ),
                ]),
                Line::from(vec![
                    Span::styled(" Avg:     ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{:.1} imports/module", stats.avg_imports),
                        Style::default().fg(Color::White),
                    ),
                ]),
                Line::from(vec![
                    Span::styled(" Leaves:  ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{}", stats.leaf_count),
                        Style::default().fg(Color::White),
                    ),
                ]),
                Line::from(vec![
                    Span::styled(" Roots:   ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{}", stats.root_count),
                        Style::default().fg(Color::White),
                    ),
                ]),
                Line::from(vec![
                    Span::styled(" Cycles:  ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{}", stats.cycle_count),
                        if stats.cycle_count > 0 {
                            Style::default().fg(Color::Red)
                        } else {
                            Style::default().fg(Color::Green)
                        },
                    ),
                ]),
                Line::from(""),
            ];

            if !stats.most_imports.is_empty() {
                lines.push(Line::from(Span::styled(
                    " Most imports (afferent coupling):",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )));
                for (m, c) in stats.most_imports.iter().take(10) {
                    if *c > 0 {
                        lines.push(Line::from(vec![
                            Span::styled(format!("   {c:>3} "), Style::default().fg(Color::Yellow)),
                            Span::styled(m, Style::default().fg(Color::White)),
                        ]));
                    }
                }
                lines.push(Line::from(""));
            }

            if !stats.most_depended_on.is_empty() {
                lines.push(Line::from(Span::styled(
                    " Most depended on (efferent coupling):",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )));
                for (m, c) in stats.most_depended_on.iter().take(10) {
                    lines.push(Line::from(vec![
                        Span::styled(format!("   {c:>3} "), Style::default().fg(Color::Yellow)),
                        Span::styled(m, Style::default().fg(Color::White)),
                    ]));
                }
            }

            lines
        }
        DepsSubView::Cycles => {
            if stats.cycles.is_empty() {
                vec![Line::from(Span::styled(
                    " No circular dependencies found.",
                    Style::default().fg(Color::Green),
                ))]
            } else {
                let mut lines = vec![Line::from(Span::styled(
                    format!(" {} circular dependency chain(s):", stats.cycle_count),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ))];
                lines.push(Line::from(""));
                for (i, cycle) in stats.cycles.iter().enumerate() {
                    lines.push(Line::from(Span::styled(
                        format!(" {}. {}", i + 1, cycle.join(" -> ")),
                        Style::default().fg(Color::Yellow),
                    )));
                }
                lines
            }
        }
        DepsSubView::Tree => {
            if let Some(ref graph_data) = state.deps.graph_data {
                let visible_height = area.height.saturating_sub(2) as usize;
                let selected = state.deps.selected_index;
                let scroll = helpers::scroll_offset(selected, visible_height);

                // Record hit test: tree view has only the top border (1 row) before data.
                state.table_hit.set(TableHitTest {
                    data_top: area.y.saturating_add(1),
                    visible_rows: visible_height as u16,
                    scroll,
                });

                let mut lines = Vec::new();

                for (i, (name, imports)) in graph_data
                    .iter()
                    .enumerate()
                    .skip(scroll)
                    .take(visible_height)
                {
                    let is_selected = i == selected;
                    let name_style = if is_selected {
                        helpers::selected_style().fg(Color::White)
                    } else {
                        Style::default().fg(Color::White)
                    };

                    if imports.is_empty() {
                        lines.push(Line::from(vec![
                            Span::styled(format!(" {name}"), name_style),
                            Span::styled(" (leaf)", Style::default().fg(Color::DarkGray)),
                        ]));
                    } else {
                        lines.push(Line::from(Span::styled(format!(" {name}"), name_style)));
                        if is_selected {
                            for imp in imports {
                                lines.push(Line::from(Span::styled(
                                    format!("   -> {imp}"),
                                    Style::default().fg(Color::DarkGray),
                                )));
                            }
                        }
                    }
                }
                lines
            } else {
                vec![Line::from(Span::styled(
                    " Loading...",
                    Style::default().fg(Color::DarkGray),
                ))]
            }
        }
    };

    let content = Paragraph::new(lines).block(block);
    frame.render_widget(content, area);
}
