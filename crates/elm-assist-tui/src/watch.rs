//! File watcher — monitors the source directory for .elm file changes
//! and sends a message to trigger re-analysis.

use std::path::Path;
use std::time::Duration;

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

use crate::app::Msg;

/// Spawn a background thread that watches for .elm and config file changes
/// and sends `Msg::FileChanged` with debouncing.
pub fn spawn_watcher(dir: &str, tx: mpsc::UnboundedSender<Msg>, debounce_ms: u64) {
    let (notify_tx, notify_rx) = std::sync::mpsc::channel::<Vec<String>>();

    let dir = dir.to_string();

    std::thread::spawn(move || {
        let mut watcher = match RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    let relevant: Vec<String> = event
                        .paths
                        .iter()
                        .filter(|p| {
                            p.extension().is_some_and(|ext| ext == "elm")
                                || p.file_name().is_some_and(|n| n == "elm-assist.toml")
                        })
                        .map(|p| p.display().to_string())
                        .collect();
                    if !relevant.is_empty() {
                        let _ = notify_tx.send(relevant);
                    }
                }
            },
            Config::default(),
        ) {
            Ok(w) => w,
            Err(e) => {
                let _ = tx.send(Msg::StatusError(format!(
                    "File watcher failed to start: {e}"
                )));
                return;
            }
        };

        if let Err(e) = watcher.watch(Path::new(&dir), RecursiveMode::Recursive) {
            let _ = tx.send(Msg::StatusError(format!(
                "File watcher failed to watch {dir}: {e}"
            )));
            return;
        }

        // Also watch the project root (parent of src_dir) non-recursively
        // so edits to `elm-assist.toml` fire events. Without this the
        // config file — which typically sits at the project root, outside
        // src/ — is never noticed and the config-changed branch in
        // `update()` is effectively dead code.
        if let Some(parent) = Path::new(&dir)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            && let Err(e) = watcher.watch(parent, RecursiveMode::NonRecursive)
        {
            // Not fatal: main watch is working, config hot-reload just won't.
            let _ = tx.send(Msg::StatusError(format!(
                "Config watcher failed (src watch still active): {e}"
            )));
        }

        while let Ok(first) = notify_rx.recv() {
            // Debounce: drain events that arrive within the configured interval.
            let mut all_paths = first;
            while let Ok(more) = notify_rx.recv_timeout(Duration::from_millis(debounce_ms)) {
                all_paths.extend(more);
            }
            all_paths.sort();
            all_paths.dedup();

            if tx.send(Msg::FileChanged(all_paths)).is_err() {
                break;
            }
        }
    });
}
