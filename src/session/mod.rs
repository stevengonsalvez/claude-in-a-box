// ABOUTME: Session management module for host tmux sessions
// Provides session lifecycle management and persistence

pub mod manager;
pub mod persistence;

pub use manager::SessionManager;
pub use persistence::SessionPersistence;