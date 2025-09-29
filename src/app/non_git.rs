// ABOUTME: Handles validation for non-git repositories
// Provides functionality to check if a directory is a git repository

use std::path::Path;
use std::process::Command;

pub fn validate_git_repository(path: &Path) -> bool {
    Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(path)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}