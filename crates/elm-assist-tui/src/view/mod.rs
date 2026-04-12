//! View dispatch — routes rendering to the active screen.

pub mod dashboard;
pub mod deps;
pub mod fix_review;
pub mod help;
pub mod helpers;
pub mod lint;
pub mod search;
pub mod status_bar;
pub mod unused;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};

use crate::app::{AppState, Screen, TableHitTest};

/// Render the current screen.
pub fn render(state: &AppState, frame: &mut Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // main content
            Constraint::Length(1), // status bar
        ])
        .split(frame.area());

    // Reset hit-test data each frame; individual table views will set it.
    state.table_hit.set(TableHitTest::default());

    match state.screen {
        Screen::Dashboard => dashboard::render(state, frame, chunks[0]),
        Screen::Lint => lint::render(state, frame, chunks[0]),
        Screen::FixReview => fix_review::render(state, frame, chunks[0]),
        Screen::Deps => deps::render(state, frame, chunks[0]),
        Screen::Unused => unused::render(state, frame, chunks[0]),
        Screen::Search => search::render(state, frame, chunks[0]),
        Screen::Help => help::render(state, frame, chunks[0]),
    }

    status_bar::render(state, frame, chunks[1]);
}
