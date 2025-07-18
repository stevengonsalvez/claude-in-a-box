// ABOUTME: Session lifecycle management that coordinates worktrees and Docker containers

use super::{ContainerManager, SessionContainer, ContainerConfig, ContainerStatus};
use crate::config::{AppConfig, ProjectConfig, ContainerTemplate, McpInitializer, apply_mcp_init_result};
use crate::git::{WorktreeManager, WorktreeInfo};
use crate::models::{Session, SessionStatus};
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;
use tracing::{info, warn};
use uuid::Uuid;
use tokio::sync::mpsc;

#[derive(Error, Debug)]
pub enum SessionLifecycleError {
    #[error("Worktree error: {0}")]
    Worktree(#[from] crate::git::WorktreeError),
    #[error("Container error: {0}")]
    Container(#[from] super::ContainerError),
    #[error("Session not found: {0}")]
    SessionNotFound(Uuid),
    #[error("Session already exists: {0}")]
    SessionAlreadyExists(Uuid),
    #[error("Invalid session state: {0}")]
    InvalidState(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

pub struct SessionLifecycleManager {
    worktree_manager: WorktreeManager,
    container_manager: ContainerManager,
    active_sessions: HashMap<Uuid, SessionState>,
    app_config: AppConfig,
}

#[derive(Debug, Clone)]
pub struct SessionState {
    pub session: Session,
    pub worktree_info: Option<WorktreeInfo>,
    pub container: Option<SessionContainer>,
}

#[derive(Debug, Clone)]
pub struct SessionRequest {
    pub session_id: Uuid,
    pub workspace_name: String,
    pub workspace_path: PathBuf,
    pub branch_name: String,
    pub base_branch: Option<String>,
    pub container_config: Option<ContainerConfig>,
}

impl SessionLifecycleManager {
    pub async fn new() -> Result<Self, SessionLifecycleError> {
        let worktree_manager = WorktreeManager::new()
            .map_err(|e| SessionLifecycleError::ConfigError(format!("Failed to create worktree manager: {}", e)))?;
        
        let container_manager = ContainerManager::new().await?;
        
        let app_config = AppConfig::load()
            .map_err(|e| SessionLifecycleError::ConfigError(format!("Failed to load config: {}", e)))?;

        Ok(Self {
            worktree_manager,
            container_manager,
            active_sessions: HashMap::new(),
            app_config,
        })
    }

    /// Create a new development session with isolated worktree and container
    pub async fn create_session(&mut self, request: SessionRequest) -> Result<SessionState, SessionLifecycleError> {
        self.create_session_with_logs(request, None).await
    }
    
    /// Create a new development session with isolated worktree and container with optional log sender
    pub async fn create_session_with_logs(&mut self, request: SessionRequest, log_sender: Option<mpsc::UnboundedSender<String>>) -> Result<SessionState, SessionLifecycleError> {
        info!("Creating new session {} for workspace {}", request.session_id, request.workspace_name);

        // Check if session already exists
        if self.active_sessions.contains_key(&request.session_id) {
            return Err(SessionLifecycleError::SessionAlreadyExists(request.session_id));
        }

        // Create worktree
        let worktree_info = self.worktree_manager.create_worktree(
            request.session_id,
            &request.workspace_path,
            &request.branch_name,
            request.base_branch.as_deref(),
        )?;

        info!("Created worktree at: {}", worktree_info.path.display());

        // Create session model
        let mut session = Session::new(
            format!("{}-{}", request.workspace_name, request.branch_name),
            request.workspace_path.to_string_lossy().to_string(),
        );
        session.id = request.session_id;
        session.branch_name = request.branch_name.clone();

        // Create container using template or provided config
        let container = if let Some(config) = request.container_config {
            // Use provided config
            let mut final_config = config;
            
            // Mount the worktree into the container
            final_config = final_config.with_volume(
                worktree_info.path.clone(),
                "/workspace".to_string(),
                false,
            );

            let container = self.container_manager
                .create_session_container(request.session_id, final_config)
                .await?;

            session.container_id = container.container_id.clone();
            Some(container)
        } else {
            // Check for project-specific config
            let project_config = ProjectConfig::load_from_dir(&request.workspace_path)
                .map_err(|e| SessionLifecycleError::ConfigError(format!("Failed to load project config: {}", e)))?;
            
            // Determine which template to use
            let template_name = project_config
                .as_ref()
                .and_then(|pc| pc.container_template.as_ref())
                .map(|s| s.as_str())
                .unwrap_or(&self.app_config.default_container_template);
            
            if let Some(template) = self.app_config.get_container_template(template_name) {
                info!("Using container template '{}' for session {}", template_name, request.session_id);
                
                // Convert template to container config
                let mut config = template.to_container_config();
                
                // Apply project-specific overrides
                if let Some(project_config) = &project_config {
                    self.apply_project_config(&mut config, project_config);
                }
                
                // Mount the worktree
                config = config.with_volume(
                    worktree_info.path.clone(),
                    "/workspace".to_string(),
                    false,
                );
                
                // Initialize MCP servers
                let mcp_initializer = McpInitializer::new(
                    McpInitializer::default_strategy(),
                    self.app_config.mcp_servers.clone(),
                );
                
                let mcp_result = mcp_initializer.initialize_for_session(
                    request.session_id,
                    &request.workspace_path,
                    &mut config,
                ).await.map_err(|e| SessionLifecycleError::ConfigError(format!("MCP initialization failed: {}", e)))?;
                
                // Apply MCP configuration to container
                apply_mcp_init_result(&mut config, &mcp_result);
                
                info!("MCP initialization completed for session {}", request.session_id);
                
                // Mount ~/.claude if requested and not already handled by MCP init
                if project_config.as_ref().map_or(true, |pc| pc.mount_claude_config) {
                    // Check if MCP init already mounted claude config
                    let already_mounted = mcp_result.volumes.iter()
                        .any(|v| v.container_path == "/home/claude-user/.claude");
                    
                    if !already_mounted {
                        if let Some(home_dir) = dirs::home_dir() {
                            let claude_dir = home_dir.join(".claude");
                            if claude_dir.exists() {
                                config = config.with_volume(
                                    claude_dir,
                                    "/home/claude-user/.claude".to_string(),
                                    false,
                                );
                            }
                        }
                    }
                }
                
                let container = self.container_manager
                    .create_session_container_with_logs(request.session_id, config, log_sender)
                    .await?;

                session.container_id = container.container_id.clone();
                Some(container)
            } else {
                warn!("Container template '{}' not found, creating session without container", template_name);
                None
            }
        };

        let session_state = SessionState {
            session,
            worktree_info: Some(worktree_info),
            container,
        };

        self.active_sessions.insert(request.session_id, session_state.clone());
        
        info!("Successfully created session {}", request.session_id);
        Ok(session_state)
    }

    /// Start a session (start the container if it exists)
    pub async fn start_session(&mut self, session_id: Uuid) -> Result<(), SessionLifecycleError> {
        info!("Starting session {}", session_id);

        let session_state = self.active_sessions
            .get_mut(&session_id)
            .ok_or(SessionLifecycleError::SessionNotFound(session_id))?;

        if let Some(ref mut container) = session_state.container {
            self.container_manager.start_container(container).await?;
            session_state.session.set_status(SessionStatus::Running);
            info!("Started container for session {}", session_id);
        } else {
            // No container, just mark as running
            session_state.session.set_status(SessionStatus::Running);
            info!("Session {} marked as running (no container)", session_id);
        }

        Ok(())
    }

    /// Stop a session (stop the container if it exists)
    pub async fn stop_session(&mut self, session_id: Uuid) -> Result<(), SessionLifecycleError> {
        info!("Stopping session {}", session_id);

        let session_state = self.active_sessions
            .get_mut(&session_id)
            .ok_or(SessionLifecycleError::SessionNotFound(session_id))?;

        if let Some(ref mut container) = session_state.container {
            self.container_manager.stop_container(container).await?;
            session_state.session.set_status(SessionStatus::Stopped);
            info!("Stopped container for session {}", session_id);
        } else {
            session_state.session.set_status(SessionStatus::Stopped);
            info!("Session {} marked as stopped (no container)", session_id);
        }

        Ok(())
    }

    /// Remove a session (cleanup worktree and container)
    pub async fn remove_session(&mut self, session_id: Uuid) -> Result<(), SessionLifecycleError> {
        info!("Removing session {}", session_id);

        let mut session_state = self.active_sessions
            .remove(&session_id)
            .ok_or(SessionLifecycleError::SessionNotFound(session_id))?;

        // Stop and remove container if it exists
        if let Some(ref mut container) = session_state.container {
            if container.is_running() {
                self.container_manager.stop_container(container).await?;
            }
            self.container_manager.remove_container(container).await?;
            info!("Removed container for session {}", session_id);
        }

        // Remove worktree
        if session_state.worktree_info.is_some() {
            self.worktree_manager.remove_worktree(session_id)?;
            info!("Removed worktree for session {}", session_id);
        }

        info!("Successfully removed session {}", session_id);
        Ok(())
    }

    /// Get session information
    pub fn get_session(&self, session_id: Uuid) -> Option<&SessionState> {
        self.active_sessions.get(&session_id)
    }

    /// List all active sessions
    pub fn list_sessions(&self) -> Vec<&SessionState> {
        self.active_sessions.values().collect()
    }

    /// Update session status by checking container status
    pub async fn refresh_session_status(&mut self, session_id: Uuid) -> Result<(), SessionLifecycleError> {
        let session_state = self.active_sessions
            .get_mut(&session_id)
            .ok_or(SessionLifecycleError::SessionNotFound(session_id))?;

        if let Some(ref mut container) = session_state.container {
            if let Some(ref container_id) = container.container_id {
                let status = self.container_manager.get_container_status(container_id).await?;
                container.status = status.clone();

                // Update session status based on container status
                session_state.session.set_status(match status {
                    ContainerStatus::Running => SessionStatus::Running,
                    ContainerStatus::Stopped | ContainerStatus::NotFound => SessionStatus::Stopped,
                    ContainerStatus::Error(msg) => SessionStatus::Error(msg),
                    _ => SessionStatus::Stopped,
                });
            }
        }

        Ok(())
    }

    /// Refresh all session statuses
    pub async fn refresh_all_sessions(&mut self) -> Result<(), SessionLifecycleError> {
        let session_ids: Vec<Uuid> = self.active_sessions.keys().copied().collect();
        
        for session_id in session_ids {
            if let Err(e) = self.refresh_session_status(session_id).await {
                warn!("Failed to refresh status for session {}: {}", session_id, e);
            }
        }

        Ok(())
    }

    /// Get container logs for a session
    pub async fn get_session_logs(&self, session_id: Uuid, lines: Option<i64>) -> Result<Vec<String>, SessionLifecycleError> {
        let session_state = self.active_sessions
            .get(&session_id)
            .ok_or(SessionLifecycleError::SessionNotFound(session_id))?;

        if let Some(ref container) = session_state.container {
            if let Some(ref container_id) = container.container_id {
                let logs = self.container_manager.get_container_logs(container_id, lines).await?;
                return Ok(logs);
            }
        }

        Ok(vec!["No container associated with this session".to_string()])
    }

    /// Get the workspace URL for a session
    pub fn get_session_workspace_url(&self, session_id: Uuid, port: u16) -> Option<String> {
        self.active_sessions
            .get(&session_id)?
            .container
            .as_ref()?
            .get_workspace_url(port)
    }

    /// Clean up orphaned sessions (sessions with missing containers or worktrees)
    pub async fn cleanup_orphaned_sessions(&mut self) -> Result<Vec<Uuid>, SessionLifecycleError> {
        let mut orphaned = Vec::new();
        let session_ids: Vec<Uuid> = self.active_sessions.keys().copied().collect();

        for session_id in session_ids {
            let mut is_orphaned = false;

            // Check if worktree still exists
            if let Err(_) = self.worktree_manager.get_worktree_info(session_id) {
                warn!("Session {} has missing worktree", session_id);
                is_orphaned = true;
            }

            // Check if container still exists (if it should)
            if let Some(session_state) = self.active_sessions.get(&session_id) {
                if let Some(ref container) = session_state.container {
                    if let Some(ref container_id) = container.container_id {
                        match self.container_manager.get_container_status(container_id).await {
                            Ok(ContainerStatus::NotFound) => {
                                warn!("Session {} has missing container", session_id);
                                is_orphaned = true;
                            }
                            Err(_) => {
                                warn!("Session {} container status check failed", session_id);
                                is_orphaned = true;
                            }
                            _ => {}
                        }
                    }
                }
            }

            if is_orphaned {
                orphaned.push(session_id);
                self.active_sessions.remove(&session_id);
            }
        }

        if !orphaned.is_empty() {
            info!("Cleaned up {} orphaned sessions", orphaned.len());
        }

        Ok(orphaned)
    }
    
    /// Apply project-specific configuration to container config
    fn apply_project_config(&self, config: &mut ContainerConfig, project_config: &ProjectConfig) {
        // Apply environment variables
        for (key, value) in &project_config.environment {
            config.environment_vars.insert(key.clone(), value.clone());
        }
        
        // Apply additional mounts
        for mount in &project_config.additional_mounts {
            *config = config.clone().with_volume(
                PathBuf::from(&mount.host_path),
                mount.container_path.clone(),
                mount.read_only,
            );
        }
        
        // Apply container config overrides if provided
        if let Some(template_config) = &project_config.container_config {
            if let Some(memory) = template_config.memory_limit {
                config.memory_limit = Some(memory * 1024 * 1024); // MB to bytes
            }
            
            if let Some(cpu) = template_config.cpu_limit {
                config.cpu_limit = Some(cpu);
            }
            
            // Add environment variables from template config
            for (key, value) in &template_config.environment {
                config.environment_vars.insert(key.clone(), value.clone());
            }
        }
    }
    
    /// Get available container templates
    pub fn get_container_templates(&self) -> &HashMap<String, ContainerTemplate> {
        &self.app_config.container_templates
    }
    
    /// Get app configuration
    pub fn get_app_config(&self) -> &AppConfig {
        &self.app_config
    }
}

impl SessionRequest {
    pub fn new(
        session_id: Uuid,
        workspace_name: String,
        workspace_path: PathBuf,
        branch_name: String,
    ) -> Self {
        Self {
            session_id,
            workspace_name,
            workspace_path,
            branch_name,
            base_branch: None,
            container_config: None,
        }
    }

    pub fn with_base_branch(mut self, base_branch: String) -> Self {
        self.base_branch = Some(base_branch);
        self
    }

    pub fn with_container_config(mut self, config: ContainerConfig) -> Self {
        self.container_config = Some(config);
        self
    }

    /// Create a request for a Claude development session
    pub fn claude_dev_session(
        session_id: Uuid,
        workspace_name: String,
        workspace_path: PathBuf,
        branch_name: String,
    ) -> Self {
        // Don't specify container_config - let the lifecycle manager use templates
        Self {
            session_id,
            workspace_name,
            workspace_path,
            branch_name,
            base_branch: None,
            container_config: None, // Will use "claude-dev" template by default
        }
    }
    
    /// Create a request with specific container template
    pub fn with_template(
        session_id: Uuid,
        workspace_name: String,
        workspace_path: PathBuf,
        branch_name: String,
        template_name: String,
    ) -> Self {
        // For now, we'll let the project config specify the template
        // In the future, we could add template selection to SessionRequest
        Self::new(session_id, workspace_name, workspace_path, branch_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::git::workspace_scanner::WorkspaceScanner;

    // Note: These tests require Docker to be running
    // They are integration tests and should be run with `cargo test --ignored`

    #[tokio::test]
    #[ignore]
    async fn test_session_lifecycle_manager_creation() {
        let manager = SessionLifecycleManager::new().await;
        assert!(manager.is_ok(), "Should be able to create session lifecycle manager");
    }

    #[tokio::test]
    #[ignore]
    async fn test_session_lifecycle() {
        let mut manager = SessionLifecycleManager::new().await.unwrap();
        let temp_dir = TempDir::new().unwrap();
        
        // Create a test git repository
        let repo = git2::Repository::init(temp_dir.path()).unwrap();
        let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        ).unwrap();

        let session_id = Uuid::new_v4();
        let request = SessionRequest::new(
            session_id,
            "test-workspace".to_string(),
            temp_dir.path().to_path_buf(),
            "test-branch".to_string(),
        );

        // Create session
        let session_state = manager.create_session(request).await.unwrap();
        assert_eq!(session_state.session.id, session_id);
        assert!(session_state.worktree_info.is_some());

        // Start session
        manager.start_session(session_id).await.unwrap();
        let session = manager.get_session(session_id).unwrap();
        assert!(session.session.status.is_running());

        // Stop session
        manager.stop_session(session_id).await.unwrap();
        let session = manager.get_session(session_id).unwrap();
        assert!(!session.session.status.is_running());

        // Remove session
        manager.remove_session(session_id).await.unwrap();
        assert!(manager.get_session(session_id).is_none());
    }
}