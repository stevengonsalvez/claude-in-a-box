// ABOUTME: Quick commit functionality for rapidly committing and pushing changes
// Provides a simple interface for git operations from the TUI

#[derive(Debug, Clone)]
pub struct QuickCommit {
    pub message: String,
    pub cursor_position: usize,
}