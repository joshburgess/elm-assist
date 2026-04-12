//! View-layer smoke tests.
//!
//! These render each screen against ratatui's `TestBackend` and assert
//! that the call doesn't panic and produces a non-empty buffer. This is
//! a cheap safety net for the dumbest class of regression — a view that
//! indexes out of range on empty state, or forgets to handle a new enum
//! variant, or panics on a newly added field.
//!
//! These tests intentionally do NOT snapshot the rendered output. The
//! view code is mostly straight-line destructuring of state into styled
//! lines; snapshot maintenance would cost more than it catches. What we
//! care about here is "does it render at all?"

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use elm_assist_tui::app::{AppState, Screen};
use elm_assist_tui::view;

fn render_screen(state: &AppState) {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("TestBackend should construct");
    terminal
        .draw(|frame| view::render(state, frame))
        .expect("render should not error");

    // Assert *something* was drawn — an empty buffer means the view
    // dispatch matched no arm or every arm returned before rendering.
    let buf = terminal.backend().buffer();
    let any_nonempty = buf
        .content
        .iter()
        .any(|cell| !cell.symbol().trim().is_empty());
    assert!(
        any_nonempty,
        "view::render produced an empty buffer for {:?}",
        state.screen
    );
}

#[test]
fn all_screens_render_on_fresh_state() {
    // Fresh state has no lint results, no search, no fix review items,
    // etc. Any view that indexes naively will panic here.
    let screens = [
        Screen::Dashboard,
        Screen::Lint,
        Screen::FixReview,
        Screen::Deps,
        Screen::Unused,
        Screen::Search,
        Screen::Help,
    ];

    for screen in screens {
        let mut state = AppState::new("src".into());
        state.screen = screen;
        render_screen(&state);
    }
}

#[test]
fn dashboard_renders_with_populated_counts() {
    let mut state = AppState::new("src".into());
    state.screen = Screen::Dashboard;
    state.module_count = 42;
    state.file_count = 42;
    state.parse_error_count = 3;
    render_screen(&state);
}

#[test]
fn status_bar_renders_info_and_error_messages() {
    let mut state = AppState::new("src".into());
    state.screen = Screen::Dashboard;
    state.status_message = Some("Lint: 5 findings in 12ms".into());
    render_screen(&state);

    state.status_message = Some("Export failed: disk full".into());
    render_screen(&state);
}

#[test]
fn help_screen_renders() {
    let mut state = AppState::new("src".into());
    state.screen = Screen::Help;
    render_screen(&state);
}

#[test]
fn tiny_terminal_does_not_panic() {
    // Pathologically small terminal — make sure layout constraints
    // don't trigger arithmetic underflow or slice panics.
    let backend = TestBackend::new(20, 5);
    let mut terminal = Terminal::new(backend).unwrap();
    let state = AppState::new("src".into());
    terminal
        .draw(|frame| view::render(&state, frame))
        .expect("render should survive tiny terminals");
}
