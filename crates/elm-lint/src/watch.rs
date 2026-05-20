use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

/// Run the lint function in a loop, re-running whenever `.elm` files change.
pub fn run_watch_loop<F>(dir: &str, mut run_lint: F) -> !
where
    F: FnMut(),
{
    let (tx, rx) = mpsc::channel();

    let tx_clone = tx.clone();
    let mut watcher = match RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                // Only forward events for .elm files.
                let is_elm = event
                    .paths
                    .iter()
                    .any(|p| p.extension().is_some_and(|ext| ext == "elm"));
                if is_elm {
                    let _ = tx_clone.send(());
                }
            }
        },
        Config::default(),
    ) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Error: failed to create file watcher: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = watcher.watch(Path::new(dir), RecursiveMode::Recursive) {
        eprintln!("Error: failed to watch directory '{dir}': {e}");
        std::process::exit(1);
    }

    // Initial run.
    clear_screen();
    run_lint();
    eprintln!("\nWatching {dir} for changes... (Ctrl+C to stop)");

    loop {
        // Block until a change is received.
        let _ = rx.recv();

        // Drain any additional events that arrived in a short window (debounce).
        while rx.recv_timeout(Duration::from_millis(50)).is_ok() {}

        clear_screen();
        run_lint();
        eprintln!("\nWatching {dir} for changes... (Ctrl+C to stop)");
    }
}

fn clear_screen() {
    eprint!("\x1B[2J\x1B[1;1H");
}
