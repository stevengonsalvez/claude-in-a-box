// ABOUTME: Git integration module for workspace detection, worktree management, and git operations

pub mod workspace_scanner;
pub mod worktree_manager;
pub mod repository;
pub mod diff_analyzer;

pub use workspace_scanner::WorkspaceScanner;
pub use worktree_manager::{WorktreeManager, WorktreeError, WorktreeInfo};
