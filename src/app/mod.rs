// ABOUTME: Main application structure and state management for the TUI

pub mod events;
pub mod session_loader;
pub mod state;

pub use events::EventHandler;
pub use session_loader::SessionLoader;
pub use state::{App, AppState};
