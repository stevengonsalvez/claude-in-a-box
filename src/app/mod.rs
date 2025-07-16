// ABOUTME: Main application structure and state management for the TUI

pub mod state;
pub mod events;
pub mod session_loader;

pub use state::{App, AppState};
pub use events::EventHandler;
pub use session_loader::SessionLoader;