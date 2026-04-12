//! Terminal event handling — polls crossterm events and sends Msgs.

use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use tokio::sync::mpsc;

use crate::app::Msg;

/// Poll crossterm events and send Msgs into the channel.
/// Runs in a dedicated tokio task.
pub async fn event_loop(tx: mpsc::UnboundedSender<Msg>) {
    loop {
        if event::poll(Duration::from_millis(100)).unwrap_or(false) {
            if let Ok(ev) = event::read() {
                let msg = match ev {
                    Event::Key(key) => {
                        // Ctrl+C always quits, regardless of mode.
                        if key.modifiers.contains(KeyModifiers::CONTROL)
                            && key.code == KeyCode::Char('c')
                        {
                            Msg::Quit
                        } else {
                            Msg::KeyPress(key)
                        }
                    }
                    Event::Mouse(mouse) => Msg::MouseEvent(mouse),
                    Event::Resize(_, _) => Msg::Tick,
                    _ => Msg::Tick,
                };
                if tx.send(msg).is_err() {
                    break;
                }
            }
        } else {
            if tx.send(Msg::Tick).is_err() {
                break;
            }
        }
    }
}
