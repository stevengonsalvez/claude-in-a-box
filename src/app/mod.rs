// ABOUTME: Main application structure and state management for the TUI

pub mod events;
pub mod non_git;
pub mod notification;
pub mod quick_commit;
pub mod session_loader;
pub mod state;
pub mod tmux_handler;

pub use events::EventHandler;
pub use session_loader::SessionLoader;
pub use state::{App, AppState};
