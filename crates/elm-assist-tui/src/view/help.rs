//! Help overlay screen — keybinding reference.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::AppState;

pub fn render(_state: &AppState, frame: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(""),
        section("Global"),
        binding("q / Ctrl+C", "Quit"),
        binding("r", "Re-run all analyses"),
        binding("?", "Toggle help"),
        binding("Esc", "Go back / clear filter"),
        Line::from(""),
        section("Navigation"),
        binding("1-5", "Switch screens (Dashboard/Lint/Deps/Unused/Search)"),
        Line::from(""),
        section("Lists"),
        binding("j/k / Up/Down", "Move selection"),
        binding("PgUp/PgDn", "Page up/down"),
        binding("g/G", "Jump to first/last"),
        binding("Mouse click", "Select row"),
        binding("Mouse scroll", "Navigate list"),
        Line::from(""),
        section("Lint Screen"),
        binding("/", "Toggle filter"),
        binding("f", "Enter fix review"),
        binding("p", "Toggle source preview"),
        binding("e", "Export diagnostics to JSON"),
        binding("* next to rule", "Auto-fix available"),
        Line::from(""),
        section("Fix Review"),
        binding("y", "Accept fix"),
        binding("n", "Skip fix"),
        binding("a", "Accept remaining (press twice to confirm)"),
        binding("q / Esc", "Quit review"),
        Line::from(""),
        section("Unused Code"),
        binding("Enter", "Toggle source preview"),
        Line::from(""),
        section("Dependencies"),
        binding("Tab", "Cycle views (Tree/Stats/Cycles)"),
        binding("j/k", "Navigate modules (Tree view)"),
        Line::from(""),
        section("Search"),
        binding("/", "Enter search query"),
        binding("Tab", "Complete query type prefix"),
        binding("Enter", "Submit query / toggle preview"),
        binding("Esc", "Cancel search input"),
    ];

    let help = Paragraph::new(lines).block(Block::default().title(" Help ").borders(Borders::ALL));

    frame.render_widget(help, area);
}

fn section(name: &str) -> Line<'static> {
    Line::from(Span::styled(
        format!("  {name}"),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))
}

fn binding(key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("    {key:<20}"), Style::default().fg(Color::Yellow)),
        Span::styled(desc.to_string(), Style::default().fg(Color::White)),
    ])
}
