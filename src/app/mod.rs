// ABOUTME: Main application structure and state management for the TUI

pub mod state;
pub mod events;

pub use state::{App, AppState};
pub use events::EventHandler;