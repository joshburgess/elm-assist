//! Dashboard screen — project health overview.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::AppState;

pub fn render(state: &AppState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title
            Constraint::Length(7), // project info
            Constraint::Length(9), // lint summary
            Constraint::Length(7), // deps summary
            Constraint::Length(5), // unused summary
            Constraint::Min(1),    // navigation help
        ])
        .split(area);

    // Title.
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            " elm-assist ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "interactive dashboard",
            Style::default().fg(Color::DarkGray),
        ),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, chunks[0]);

    // Project info.
    let mut project_lines = vec![
        Line::from(vec![
            Span::styled(" Source: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&state.src_dir, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" Files:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", state.file_count),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Modules:", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(" {}", state.module_count),
                Style::default().fg(Color::White),
            ),
        ]),
    ];
    if state.parse_error_count > 0 {
        project_lines.push(Line::from(vec![
            Span::styled(" Parse errors: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", state.parse_error_count),
                Style::default().fg(Color::Red),
            ),
        ]));
    }
    let project = Paragraph::new(project_lines)
        .block(Block::default().title(" Project ").borders(Borders::ALL));
    frame.render_widget(project, chunks[1]);

    // Lint summary.
    let lint_lines = if let Some(ref result) = state.lint.result {
        let error_style = if result.total_errors > 0 {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Green)
        };
        let fixable_style = if result.total_fixable > 0 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        vec![
            Line::from(vec![
                Span::styled(" Findings: ", Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{}", result.total_errors), error_style),
            ]),
            Line::from(vec![
                Span::styled(" Warnings: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}", result.total_warnings),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(vec![
                Span::styled(" Fixable:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{}", result.total_fixable), fixable_style),
            ]),
            Line::from(vec![
                Span::styled(" Rules:    ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}", result.rules_active),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled(" Time:     ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:.1}ms", result.elapsed.as_secs_f64() * 1000.0),
                    Style::default().fg(Color::White),
                ),
                if result.cached {
                    Span::styled(" (cached)", Style::default().fg(Color::DarkGray))
                } else {
                    Span::raw("")
                },
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            " Waiting for lint results...",
            Style::default().fg(Color::DarkGray),
        ))]
    };
    let lint =
        Paragraph::new(lint_lines).block(Block::default().title(" Lint ").borders(Borders::ALL));
    frame.render_widget(lint, chunks[2]);

    // Deps summary.
    let deps_lines = if let Some(ref stats) = state.deps.stats {
        vec![
            Line::from(vec![
                Span::styled(" Modules: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}", stats.total_modules),
                    Style::default().fg(Color::White),
                ),
                Span::styled("  Edges: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}", stats.total_edges),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled(" Avg imports: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:.1}", stats.avg_imports),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled(" Cycles: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}", stats.cycle_count),
                    if stats.cycle_count > 0 {
                        Style::default().fg(Color::Red)
                    } else {
                        Style::default().fg(Color::Green)
                    },
                ),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            " Analyzing dependencies...",
            Style::default().fg(Color::DarkGray),
        ))]
    };
    let deps = Paragraph::new(deps_lines).block(
        Block::default()
            .title(" Dependencies ")
            .borders(Borders::ALL),
    );
    frame.render_widget(deps, chunks[3]);

    // Unused summary.
    let unused_lines = vec![Line::from(vec![
        Span::styled(" Findings: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}", state.unused.findings.len()),
            if state.unused.findings.is_empty() {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Yellow)
            },
        ),
    ])];
    let unused = Paragraph::new(unused_lines).block(
        Block::default()
            .title(" Unused Code ")
            .borders(Borders::ALL),
    );
    frame.render_widget(unused, chunks[4]);

    // Navigation help.
    let nav = Paragraph::new(vec![Line::from(vec![
        Span::styled(
            " 1",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Dashboard  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "2",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Lint  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "3",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Deps  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "4",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Unused  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "5",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Search  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "?",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Help  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "q",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Quit", Style::default().fg(Color::DarkGray)),
    ])]);
    frame.render_widget(nav, chunks[5]);
}
