// ABOUTME: Error types for tmux session management
// Defines error conditions that can occur when managing host tmux sessions

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TmuxError {
    #[error("PTY creation failed: {0}")]
    PtyCreationFailed(String),

    #[error("Tmux not installed on host")]
    TmuxNotInstalled,

    #[error("Session already exists: {0}")]
    SessionExists(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Attach failed: {0}")]
    AttachFailed(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Nix error: {0}")]
    NixError(#[from] nix::Error),
}
