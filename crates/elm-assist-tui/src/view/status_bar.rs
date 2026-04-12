//! Status bar at the bottom of the screen.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{AppState, InputMode, Screen};

pub fn render(state: &AppState, frame: &mut Frame, area: Rect) {
    let screen_name = match state.screen {
        Screen::Dashboard => "Dashboard",
        Screen::Lint => "Lint",
        Screen::FixReview => "Fix Review",
        Screen::Deps => "Dependencies",
        Screen::Unused => "Unused Code",
        Screen::Search => "Search",
        Screen::Help => "Help",
    };

    let mut spans = vec![
        Span::styled(
            format!(" {screen_name} "),
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ", Style::default()),
    ];

    // Show input mode indicator.
    match state.input_mode {
        InputMode::LintFilter => {
            spans.push(Span::styled(
                "FILTER ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                " Type to filter, Esc/Enter to confirm ",
                Style::default().fg(Color::Yellow),
            ));
        }
        InputMode::SearchQuery => {
            spans.push(Span::styled(
                "SEARCH ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                " Type query, Enter to search, Esc to cancel ",
                Style::default().fg(Color::Yellow),
            ));
        }
        InputMode::Normal => {
            if let Some(ref msg) = state.status_message {
                spans.push(Span::styled(msg, Style::default().fg(Color::Yellow)));
            } else if state.loading {
                spans.push(Span::styled(
                    "Loading...",
                    Style::default().fg(Color::Yellow),
                ));
            }
        }
    }

    let bar = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::DarkGray));

    frame.render_widget(bar, area);
}
