// ABOUTME: Session loader that queries tmux sessions and worktrees to load active sessions
// Groups sessions by their source repository for display

use crate::config::AppConfig;
use crate::git::WorktreeManager;
use crate::models::{Session, SessionStatus, Workspace};
use crate::tmux::TmuxSession;
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, info, warn};
use uuid::Uuid;

pub struct SessionLoader {
    worktree_manager: WorktreeManager,
    config: AppConfig,
}

impl SessionLoader {
    pub async fn new() -> Result<Self> {
        let worktree_manager = WorktreeManager::new()?;
        let config = AppConfig::load()?;

        Ok(Self {
            worktree_manager,
            config,
        })
    }

    /// Extract repository name from tmux session name for orphaned sessions
    /// This helps display meaningful workspace names instead of showing username
    fn extract_repo_name_from_tmux_session(tmux_name: &str) -> Option<String> {
        // Remove ciab_ prefix if present
        let name_part = tmux_name.strip_prefix("ciab_").unwrap_or(tmux_name);

        // Look for known repository patterns
        if name_part.contains("claude_in_a_box") || name_part.contains("claude-in-a-box") {
            return Some("claude-in-a-box".to_string());
        }

        // Could add more patterns for other repositories as needed
        // For now, only handle claude-in-a-box explicitly

        None
    }

    /// Load all active sessions from tmux and worktrees
    pub async fn load_active_sessions(&self) -> Result<Vec<Workspace>> {
        info!("Loading active sessions from tmux");

        // Get list of running tmux sessions
        let tmux_sessions = TmuxSession::list_sessions().await.unwrap_or_default();
        info!("Found {} tmux sessions", tmux_sessions.len());

        // Load all worktrees
        let worktrees_list = self.worktree_manager.list_all_worktrees()
            .unwrap_or_default();

        // Convert to HashMap for easier lookup
        let worktrees: HashMap<Uuid, crate::git::WorktreeInfo> = worktrees_list
            .into_iter()
            .collect();

        // Group sessions by source repository
        let mut workspace_map: HashMap<String, Vec<Session>> = HashMap::new();

        // Process tmux sessions
        for tmux_name in &tmux_sessions {
            // Extract session info from tmux name (format: ciab_workspace_timestamp)
            if let Some(session) = self.create_session_from_tmux(&tmux_name, &worktrees).await {
                let workspace_key = session.workspace_path.clone();
                workspace_map.entry(workspace_key)
                    .or_insert_with(Vec::new)
                    .push(session);
            }
        }

        // Also add orphaned worktrees as stopped sessions
        for (id, worktree_info) in &worktrees {
            let worktree_name_part = crate::models::Session::sanitize_tmux_name(&worktree_info.branch_name);
            let has_tmux = tmux_sessions.iter().any(|t| t.contains(&worktree_name_part));

            if !has_tmux {
                if let Some(session) = self.create_session_from_worktree(*id, worktree_info).await {
                    let workspace_key = session.workspace_path.clone();
                    workspace_map.entry(workspace_key)
                        .or_insert_with(Vec::new)
                        .push(session);
                }
            }
        }

        // Convert to workspace format
        let workspaces: Vec<Workspace> = workspace_map
            .into_iter()
            .map(|(path, sessions)| {
                let name = Path::new(&path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                Workspace {
                    name,
                    path: PathBuf::from(path),
                    sessions,
                }
            })
            .collect();

        info!(
            "Loaded {} workspaces with {} total sessions",
            workspaces.len(),
            workspaces.iter().map(|w| w.sessions.len()).sum::<usize>()
        );
        Ok(workspaces)
    }

    async fn create_session_from_tmux(&self, tmux_name: &str, worktrees: &HashMap<Uuid, crate::git::WorktreeInfo>) -> Option<Session> {
        // Parse tmux session name to extract details
        let name_without_prefix = tmux_name.strip_prefix("ciab_").unwrap_or(tmux_name);

        // Find matching worktree
        let matching_worktree = worktrees.iter()
            .find(|(_, info)| {
                let worktree_name_part = crate::models::Session::sanitize_tmux_name(&info.branch_name);
                name_without_prefix.contains(&worktree_name_part)
            });

        if let Some((id, worktree_info)) = matching_worktree {
            let mut session = Session::new(
                worktree_info.branch_name.clone(),
                worktree_info.source_repository.to_string_lossy().to_string()
            );
            session.id = *id;
            session.tmux_session_name = tmux_name.to_string();
            session.worktree_path = worktree_info.path.to_string_lossy().to_string();
            session.branch_name = worktree_info.branch_name.clone();
            session.status = SessionStatus::Running; // Tmux session exists, so it's running

            // Check if attached
            if let Ok(output) = Command::new("tmux")
                .args(&["list-clients", "-t", tmux_name])
                .output() {
                if output.status.success() && !output.stdout.is_empty() {
                    session.status = SessionStatus::Attached;
                }
            }

            Some(session)
        } else {
            // Tmux session without matching worktree - create minimal session
            // Try to infer repository name from tmux session name first
            let workspace_path = if let Some(inferred_repo) = Self::extract_repo_name_from_tmux_session(tmux_name) {
                // Use inferred repository name as a synthetic workspace path
                // This will make the workspace name display as the repository name instead of username
                format!("/synthetic/{}", inferred_repo)
            } else {
                // Fall back to home directory if no repository can be inferred
                dirs::home_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "/".to_string())
            };

            let mut session = Session::new(
                name_without_prefix.to_string(),
                workspace_path
            );
            session.tmux_session_name = tmux_name.to_string();
            session.status = SessionStatus::Running;
            Some(session)
        }
    }

    async fn create_session_from_worktree(&self, id: Uuid, worktree_info: &crate::git::WorktreeInfo) -> Option<Session> {
        let mut session = Session::new(
            worktree_info.branch_name.clone(),
            worktree_info.source_repository.to_string_lossy().to_string(),
        );
        session.id = id;
        session.worktree_path = worktree_info.path.to_string_lossy().to_string();
        session.branch_name = worktree_info.branch_name.clone();
        session.status = SessionStatus::Stopped; // No tmux session
        session.tmux_session_name = format!("ciab_{}", crate::models::Session::sanitize_tmux_name(&worktree_info.branch_name));

        Some(session)
    }

    /// Load sessions from persistence (e.g., ~/.claude-box/sessions.json)
    pub async fn load_from_persistence(&self) -> Result<Vec<Session>> {
        // TODO: Implement loading from ~/.claude-box/sessions.json
        // For now, return empty vec
        Ok(vec![])
    }

    /// Create a new session browser to select repository for new session
    pub async fn get_available_repositories(&self) -> Result<Vec<PathBuf>> {
        // Use workspace scanner to find repositories
        use crate::git::WorkspaceScanner;

        let scanner = WorkspaceScanner::with_additional_paths(
            self.config.workspace_defaults.workspace_scan_paths.clone(),
        );
        let scan_result = scanner.scan()?;

        let repos: Vec<PathBuf> = scan_result
            .workspaces
            .into_iter()
            .map(|w| w.path)
            .take(100) // Limit to 100 repositories to prevent UI performance issues
            .collect();

        info!(
            "Found {} repositories (limited to 100 for UI performance)",
            repos.len()
        );
        Ok(repos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_loader_creation() {
        let loader = SessionLoader::new().await;
        assert!(loader.is_ok());
    }
}
