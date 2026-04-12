//! elm-assist-tui — library exports for the interactive terminal dashboard.
//!
//! The binary entry point is in main.rs. This module re-exports the core
//! TEA (The Elm Architecture) types and functions so integration tests can
//! exercise the state machine without needing a terminal.

pub mod app;
pub mod command;
pub mod event;
pub mod view;
pub mod watch;
