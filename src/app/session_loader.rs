// ABOUTME: Session loader that queries Docker containers and worktrees to load active sessions
// Groups sessions by their source repository for display

use crate::docker::ContainerManager;
use crate::git::WorktreeManager;
use crate::models::{Session, SessionStatus, Workspace};
use crate::config::AppConfig;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use uuid::Uuid;

pub struct SessionLoader {
    container_manager: ContainerManager,
    worktree_manager: WorktreeManager,
    config: AppConfig,
}

impl SessionLoader {
    pub async fn new() -> Result<Self> {
        let container_manager = ContainerManager::new().await?;
        let worktree_manager = WorktreeManager::new()?;
        let config = AppConfig::load()?;
        
        Ok(Self {
            container_manager,
            worktree_manager,
            config,
        })
    }
    
    /// Load all active sessions from Docker containers and worktrees
    pub async fn load_active_sessions(&self) -> Result<Vec<Workspace>> {
        info!("Loading active sessions from Docker containers");
        
        // Get all Claude-managed containers
        let containers = self.container_manager.list_claude_containers().await?;
        info!("Found {} Claude-managed containers", containers.len());
        
        // Group sessions by their source repository
        let mut workspace_map: HashMap<PathBuf, Workspace> = HashMap::new();
        
        for container in containers {
            // Extract session ID from container labels
            let session_id = container.labels
                .as_ref()
                .and_then(|labels| labels.get("claude-session-id"))
                .and_then(|id| Uuid::parse_str(id).ok());
            
            if let Some(session_id) = session_id {
                debug!("Processing container for session {}", session_id);
                
                // Get worktree information for this session
                match self.worktree_manager.get_worktree_info(session_id) {
                    Ok(worktree_info) => {
                        // Create session from container and worktree info
                        let mut session = Session::new(
                            worktree_info.branch_name.clone(),
                            worktree_info.source_repository.to_string_lossy().to_string(),
                        );
                        session.id = session_id;
                        session.container_id = container.id;
                        session.branch_name = worktree_info.branch_name.clone();
                        
                        // Set session status based on container state
                        let state = container.state.as_deref().unwrap_or("unknown");
                        session.set_status(match state {
                            "running" => SessionStatus::Running,
                            "paused" => SessionStatus::Stopped,
                            "exited" | "dead" => SessionStatus::Stopped,
                            _ => SessionStatus::Error(format!("Unknown container state: {}", state)),
                        });
                        
                        // Get workspace name from source repo
                        let workspace_name = worktree_info.source_repository
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        
                        // Add session to appropriate workspace
                        let workspace = workspace_map
                            .entry(worktree_info.source_repository.clone())
                            .or_insert_with(|| Workspace::new(
                                workspace_name,
                                worktree_info.source_repository.clone(),
                            ));
                        
                        workspace.add_session(session);
                    }
                    Err(e) => {
                        warn!("Failed to get worktree info for session {}: {}", session_id, e);
                        // Container exists but worktree is missing - this is an orphaned container
                    }
                }
            } else {
                warn!("Container {} has no session ID label", container.id.unwrap_or_default());
            }
        }
        
        // Also check for worktrees without containers (orphaned worktrees)
        match self.worktree_manager.list_all_worktrees() {
            Ok(worktree_list) => {
                for (session_id, worktree_info) in worktree_list {
                    // Check if we already processed this session from containers
                    let already_processed = workspace_map.values()
                        .any(|w| w.sessions.iter().any(|s| s.id == session_id));
                    
                    if !already_processed {
                        debug!("Found orphaned worktree for session {}", session_id);
                        
                        // Create session for orphaned worktree
                        let mut session = Session::new(
                            worktree_info.branch_name.clone(),
                            worktree_info.source_repository.to_string_lossy().to_string(),
                        );
                        session.id = session_id;
                        session.branch_name = worktree_info.branch_name.clone();
                        session.set_status(SessionStatus::Stopped); // No container = stopped
                        
                        let workspace_name = worktree_info.source_repository
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        
                        let workspace = workspace_map
                            .entry(worktree_info.source_repository.clone())
                            .or_insert_with(|| Workspace::new(
                                workspace_name,
                                worktree_info.source_repository.clone(),
                            ));
                        
                        workspace.add_session(session);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to list worktrees: {}", e);
            }
        }
        
        // Convert map to sorted vector
        let mut workspaces: Vec<Workspace> = workspace_map.into_values().collect();
        workspaces.sort_by(|a, b| a.name.cmp(&b.name));
        
        info!("Loaded {} workspaces with active sessions", workspaces.len());
        Ok(workspaces)
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
            self.config.workspace_defaults.workspace_scan_paths.clone()
        );
        let scan_result = scanner.scan()?;
        
        let repos: Vec<PathBuf> = scan_result.workspaces
            .into_iter()
            .map(|w| w.path)
            .take(100)  // Limit to 100 repositories to prevent UI performance issues
            .collect();
        
        info!("Found {} repositories (limited to 100 for UI performance)", repos.len());
        Ok(repos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[ignore] // Requires Docker
    async fn test_session_loader_creation() {
        let loader = SessionLoader::new().await;
        assert!(loader.is_ok());
    }
}