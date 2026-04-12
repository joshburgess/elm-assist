//! elm-assist-tui — interactive terminal dashboard for the elm-assist toolchain.

use std::io;
use std::panic;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;

use elm_assist_tui::app::{AppState, Command, Msg};
use elm_assist_tui::{command, event, view, watch};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse args.
    let args: Vec<String> = std::env::args().collect();
    let src_dir = args.get(1).map(|s| s.as_str()).unwrap_or("src");

    if !std::path::Path::new(src_dir).exists() {
        eprintln!("Error: directory '{src_dir}' not found.");
        std::process::exit(1);
    }

    // Install panic hook that restores terminal before printing panic.
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(info);
    }));

    // Setup terminal.
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Run the app, capturing any error so we can always restore the
    // terminal before returning. A plain `?` here would bypass cleanup
    // and leave the user's terminal in raw/alternate-screen mode.
    let run_result = run(&mut terminal, src_dir).await;

    // Restore terminal unconditionally.
    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    );
    let _ = terminal.show_cursor();

    run_result
}

async fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    src_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create message channel.
    let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();

    // Spawn event polling task.
    let event_tx = tx.clone();
    tokio::spawn(async move {
        event::event_loop(event_tx).await;
    });

    // Load config for TUI settings.
    let config = elm_lint::config::Config::discover()
        .map(|(_, c)| c)
        .unwrap_or_default();
    let debounce_ms = config.tui.debounce_ms;

    // Initialize state.
    let mut state = AppState::new(src_dir.to_string());

    // Start file watcher.
    watch::spawn_watcher(src_dir, tx.clone(), debounce_ms);

    // Kick off initial project scan.
    let cmd_tx = tx.clone();
    let cmd_dir = src_dir.to_string();
    tokio::spawn(async move {
        command::execute(Command::ScanProject, cmd_dir, cmd_tx).await;
    });

    // Main loop.
    loop {
        // Render.
        terminal.draw(|frame| {
            view::render(&state, frame);
        })?;

        // Wait for next message.
        if let Some(msg) = rx.recv().await {
            let cmd = elm_assist_tui::app::update(&mut state, msg);

            if state.quit {
                break;
            }

            // Execute commands.
            if !matches!(cmd, Command::None) {
                let cmd_tx = tx.clone();
                let cmd_dir = state.src_dir.clone();
                tokio::spawn(async move {
                    command::execute(cmd, cmd_dir, cmd_tx).await;
                });
            }
        } else {
            break;
        }
    }

    Ok(())
}
