// ABOUTME: Application state management and view switching logic

use crate::models::{Session, Workspace};
use crate::app::SessionLoader;
use std::collections::HashMap;
use uuid::Uuid;
use tracing::{warn, info, error};
use tokio::sync::mpsc;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    SessionList,
    Logs,
    Terminal,
    Help,
    NewSession,
    SearchWorkspace,
    NonGitNotification,
    AttachedTerminal,
    AuthSetup,  // New view for authentication setup
}

#[derive(Debug, Clone)]
pub struct ConfirmationDialog {
    pub title: String,
    pub message: String,
    pub confirm_action: ConfirmAction,
    pub selected_option: bool, // true = Yes, false = No
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    DeleteSession(Uuid),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMethod {
    OAuth,
    ApiKey,
    Skip,
}

#[derive(Debug, Clone)]
pub struct AuthSetupState {
    pub selected_method: AuthMethod,
    pub api_key_input: String,
    pub is_processing: bool,
    pub error_message: Option<String>,
    pub show_cursor: bool,
}

#[derive(Debug)]
pub struct AppState {
    pub workspaces: Vec<Workspace>,
    pub selected_workspace_index: Option<usize>,
    pub selected_session_index: Option<usize>,
    pub current_view: View,
    pub should_quit: bool,
    pub logs: HashMap<Uuid, Vec<String>>,
    pub help_visible: bool,
    // New session creation state
    pub new_session_state: Option<NewSessionState>,
    // Async action processing
    pub pending_async_action: Option<AsyncAction>,
    // Flag to track if user cancelled during async operation
    pub async_operation_cancelled: bool,
    // Confirmation dialog state
    pub confirmation_dialog: Option<ConfirmationDialog>,
    // Flag to force UI refresh after workspace changes
    pub ui_needs_refresh: bool,
    // Track if current directory is a git repository
    pub is_current_dir_git_repo: bool,
    // Track which session logs were last fetched to avoid unnecessary refetches
    pub last_logs_session_id: Option<Uuid>,
    // Track attached terminal state
    pub attached_session_id: Option<Uuid>,
    // Auth setup state
    pub auth_setup_state: Option<AuthSetupState>,
}

#[derive(Debug)]
pub struct NewSessionState {
    pub available_repos: Vec<std::path::PathBuf>,
    pub filtered_repos: Vec<(usize, std::path::PathBuf)>, // (original_index, path)
    pub selected_repo_index: Option<usize>,
    pub branch_name: String,
    pub step: NewSessionStep,
    pub filter_text: String,
    pub is_current_dir_mode: bool, // true if creating session in current dir
}

impl NewSessionState {
    pub fn apply_filter(&mut self) {
        self.filtered_repos.clear();
        let filter_lower = self.filter_text.to_lowercase();
        
        for (idx, repo) in self.available_repos.iter().enumerate() {
            if let Some(folder_name) = repo.file_name() {
                if let Some(name_str) = folder_name.to_str() {
                    if name_str.to_lowercase().contains(&filter_lower) {
                        self.filtered_repos.push((idx, repo.clone()));
                    }
                }
            }
        }
        
        // Reset selection if current selection is out of bounds
        if let Some(idx) = self.selected_repo_index {
            if idx >= self.filtered_repos.len() {
                self.selected_repo_index = if self.filtered_repos.is_empty() {
                    None
                } else {
                    Some(0)
                };
            }
        } else if !self.filtered_repos.is_empty() {
            self.selected_repo_index = Some(0);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewSessionStep {
    SelectRepo,
    InputBranch,
    Creating,
}

#[derive(Debug, Clone)]
pub enum AsyncAction {
    StartNewSession,       // Old - will be removed
    StartWorkspaceSearch,   // New - search all workspaces
    NewSessionInCurrentDir, // New - create session in current directory
    CreateNewSession,
    DeleteSession(Uuid),   // New - delete session with container cleanup
    RefreshWorkspaces,     // Manual refresh of workspace data
    FetchContainerLogs(Uuid), // Fetch container logs for a session
    AttachToContainer(Uuid), // Attach to a container session
    KillContainer(Uuid), // Kill container for a session
    AuthSetupOAuth,        // Run OAuth authentication setup
    AuthSetupApiKey,       // Save API key authentication
    ReauthenticateCredentials, // Re-authenticate Claude credentials
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            workspaces: Vec::new(),
            selected_workspace_index: None,
            selected_session_index: None,
            current_view: View::SessionList,
            should_quit: false,
            logs: HashMap::new(),
            help_visible: false,
            new_session_state: None,
            pending_async_action: None,
            async_operation_cancelled: false,
            confirmation_dialog: None,
            ui_needs_refresh: false,
            is_current_dir_git_repo: false,
            last_logs_session_id: None,
            attached_session_id: None,
            auth_setup_state: None,
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Check if this is first time setup (no auth configured)
    pub fn is_first_time_setup() -> bool {
        let home_dir = match dirs::home_dir() {
            Some(dir) => dir,
            None => return false,
        };
        
        let auth_dir = home_dir.join(".claude-in-a-box/auth");
        let has_credentials = auth_dir.join(".credentials.json").exists();
        let has_claude_json = auth_dir.join(".claude.json").exists();
        let has_api_key = std::env::var("ANTHROPIC_API_KEY").is_ok();
        let has_env_file = home_dir.join(".claude-in-a-box/.env").exists();
        
        // Load .env file if it exists to check for API key
        let has_env_api_key = if has_env_file {
            if let Ok(contents) = std::fs::read_to_string(home_dir.join(".claude-in-a-box/.env")) {
                contents.contains("ANTHROPIC_API_KEY=")
            } else {
                false
            }
        } else {
            false
        };
        
        // For OAuth authentication, we need BOTH .credentials.json AND .claude.json
        let has_complete_oauth = has_credentials && has_claude_json;
        
        // Show auth screen if we don't have complete OAuth setup AND no API key alternatives
        !has_complete_oauth && !has_api_key && !has_env_api_key
    }

    pub fn check_current_directory_status(&mut self) {
        use std::env;
        use crate::git::workspace_scanner::WorkspaceScanner;
        
        if let Ok(current_dir) = env::current_dir() {
            self.is_current_dir_git_repo = WorkspaceScanner::validate_workspace(&current_dir).unwrap_or(false);
            
            if !self.is_current_dir_git_repo {
                info!("Current directory is not a git repository: {:?}", current_dir);
                self.current_view = View::NonGitNotification;
            } else {
                info!("Current directory is a valid git repository: {:?}", current_dir);
            }
        } else {
            warn!("Could not determine current directory");
            self.is_current_dir_git_repo = false;
            self.current_view = View::NonGitNotification;
        }
    }

    pub async fn load_real_workspaces(&mut self) {
        info!("Loading active sessions from Docker containers");
        
        // Try to load active sessions
        match SessionLoader::new().await {
            Ok(loader) => {
                match loader.load_active_sessions().await {
                    Ok(workspaces) => {
                        self.workspaces = workspaces;
                        info!("Loaded {} workspaces with active sessions", self.workspaces.len());
                        
                        // Queue logs fetch for the currently selected session if any
                        self.queue_logs_fetch();
                        
                        // Set initial selection
                        if !self.workspaces.is_empty() {
                            self.selected_workspace_index = Some(0);
                            if !self.workspaces[0].sessions.is_empty() {
                                self.selected_session_index = Some(0);
                            }
                        } else {
                            info!("No active sessions found. Use 'n' to create a new session.");
                            self.selected_workspace_index = None;
                            self.selected_session_index = None;
                        }
                    }
                    Err(e) => {
                        warn!("Failed to load active sessions: {}", e);
                        info!("No active sessions found. Use 'n' to create a new session.");
                        self.workspaces.clear();
                        self.selected_workspace_index = None;
                        self.selected_session_index = None;
                    }
                }
            }
            Err(e) => {
                warn!("Failed to create session loader: {}", e);
                info!("No active sessions found. Use 'n' to create a new session.");
                self.workspaces.clear();
                self.selected_workspace_index = None;
                self.selected_session_index = None;
            }
        }
    }

    pub fn load_mock_data(&mut self) {
        let mut workspace1 = Workspace::new(
            "project1".to_string(),
            "/Users/user/projects/project1".into(),
        );
        
        let mut session1 = Session::new("fix-auth".to_string(), workspace1.path.to_string_lossy().to_string());
        session1.set_status(crate::models::SessionStatus::Running);
        session1.git_changes.added = 42;
        session1.git_changes.deleted = 13;
        
        let mut session2 = Session::new("add-feature".to_string(), workspace1.path.to_string_lossy().to_string());
        session2.set_status(crate::models::SessionStatus::Stopped);
        
        let mut session3 = Session::new("debug-issue".to_string(), workspace1.path.to_string_lossy().to_string());
        session3.set_status(crate::models::SessionStatus::Error("Container failed to start".to_string()));
        
        workspace1.add_session(session1);
        workspace1.add_session(session2);
        workspace1.add_session(session3);
        
        let mut workspace2 = Workspace::new(
            "project2".to_string(),
            "/Users/user/projects/project2".into(),
        );
        
        let mut session4 = Session::new("refactor-api".to_string(), workspace2.path.to_string_lossy().to_string());
        session4.set_status(crate::models::SessionStatus::Running);
        session4.git_changes.modified = 7;
        
        workspace2.add_session(session4);
        
        self.workspaces.push(workspace1);
        self.workspaces.push(workspace2);
        
        if !self.workspaces.is_empty() {
            self.selected_workspace_index = Some(0);
            if !self.workspaces[0].sessions.is_empty() {
                self.selected_session_index = Some(0);
            }
        }
    }

    /// Load a large dataset to simulate the 353 repository scenario
    pub fn load_large_mock_data(&mut self) {
        // Load normal mock data first
        self.load_mock_data();
        
        // Add many more workspaces to simulate large dataset
        for i in 3..=200 {
            let workspace = Workspace::new(
                format!("test-project-{:03}", i),
                format!("/Users/user/projects/test-project-{:03}", i).into(),
            );
            self.workspaces.push(workspace);
        }
        
        info!("Loaded large mock dataset with {} workspaces", self.workspaces.len());
    }

    pub fn selected_session(&self) -> Option<&Session> {
        let workspace_idx = self.selected_workspace_index?;
        let session_idx = self.selected_session_index?;
        self.workspaces.get(workspace_idx)?.sessions.get(session_idx)
    }

    pub fn selected_workspace(&self) -> Option<&Workspace> {
        let workspace_idx = self.selected_workspace_index?;
        self.workspaces.get(workspace_idx)
    }

    pub fn next_session(&mut self) {
        if let Some(workspace_idx) = self.selected_workspace_index {
            if let Some(workspace) = self.workspaces.get(workspace_idx) {
                if !workspace.sessions.is_empty() {
                    let current = self.selected_session_index.unwrap_or(0);
                    self.selected_session_index = Some((current + 1) % workspace.sessions.len());
                    // Queue container logs fetch for the newly selected session
                    self.queue_logs_fetch();
                }
            }
        }
    }

    pub fn previous_session(&mut self) {
        if let Some(workspace_idx) = self.selected_workspace_index {
            if let Some(workspace) = self.workspaces.get(workspace_idx) {
                if !workspace.sessions.is_empty() {
                    let current = self.selected_session_index.unwrap_or(0);
                    self.selected_session_index = Some(
                        if current == 0 {
                            workspace.sessions.len() - 1
                        } else {
                            current - 1
                        }
                    );
                    // Queue container logs fetch for the newly selected session
                    self.queue_logs_fetch();
                }
            }
        }
    }

    pub fn next_workspace(&mut self) {
        if !self.workspaces.is_empty() {
            let current = self.selected_workspace_index.unwrap_or(0);
            self.selected_workspace_index = Some((current + 1) % self.workspaces.len());
            self.selected_session_index = if !self.workspaces[self.selected_workspace_index.unwrap()].sessions.is_empty() {
                Some(0)
            } else {
                None
            };
            // Queue container logs fetch for the newly selected session
            self.queue_logs_fetch();
        }
    }

    pub fn previous_workspace(&mut self) {
        if !self.workspaces.is_empty() {
            let current = self.selected_workspace_index.unwrap_or(0);
            self.selected_workspace_index = Some(
                if current == 0 {
                    self.workspaces.len() - 1
                } else {
                    current - 1
                }
            );
            self.selected_session_index = if !self.workspaces[self.selected_workspace_index.unwrap()].sessions.is_empty() {
                Some(0)
            } else {
                None
            };
            // Queue container logs fetch for the newly selected session
            self.queue_logs_fetch();
        }
    }

    pub fn toggle_help(&mut self) {
        self.help_visible = !self.help_visible;
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn show_delete_confirmation(&mut self, session_id: Uuid) {
        self.confirmation_dialog = Some(ConfirmationDialog {
            title: "Delete Session".to_string(),
            message: "Are you sure you want to delete this session? This will stop the container and remove the git worktree.".to_string(),
            confirm_action: ConfirmAction::DeleteSession(session_id),
            selected_option: false, // Default to "No"
        });
    }

    /// Queue fetching container logs for the currently selected session if needed
    fn queue_logs_fetch(&mut self) {
        // Get session ID without borrowing self
        if let Some(session_id) = self.get_selected_session_id() {
            // Only fetch if we haven't already fetched logs for this session
            if self.last_logs_session_id != Some(session_id) {
                self.pending_async_action = Some(AsyncAction::FetchContainerLogs(session_id));
                self.last_logs_session_id = Some(session_id);
            }
        }
    }
    
    /// Get the ID of the currently selected session without borrowing self
    pub fn get_selected_session_id(&self) -> Option<Uuid> {
        let workspace_idx = self.selected_workspace_index?;
        let session_idx = self.selected_session_index?;
        self.workspaces.get(workspace_idx)?.sessions.get(session_idx).map(|s| s.id)
    }

    /// Attach to a container session using docker exec with proper terminal handling
    pub async fn attach_to_container(&mut self, session_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        use crate::docker::ContainerManager;
        
        // Find the session to get container ID
        let container_id = self.workspaces
            .iter()
            .flat_map(|w| &w.sessions)
            .find(|s| s.id == session_id)
            .and_then(|s| s.container_id.as_ref())
            .cloned();
        
        if let Some(container_id) = container_id {
            info!("Attaching to container {} for session {}", container_id, session_id);
            
            // Check if container is running
            let container_manager = ContainerManager::new().await?;
            let status = container_manager.get_container_status(&container_id).await?;
            
            match status {
                crate::docker::ContainerStatus::Running => {
                    // Start Claude CLI directly using docker exec with proper terminal handling
                    let exec_command = vec![
                        "claude".to_string(),
                        "--dangerously-skip-permissions".to_string(),
                    ];
                    
                    match container_manager.exec_interactive_blocking(&container_id, exec_command).await {
                        Ok(_exit_status) => {
                            info!("Successfully detached from container {} for session {}", container_id, session_id);
                            // The container session has ended, stay in current view
                            Ok(())
                        }
                        Err(e) => {
                            error!("Failed to exec into container {}: {}", container_id, e);
                            Err(format!("Failed to attach to container: {}", e).into())
                        }
                    }
                }
                _ => {
                    warn!("Cannot attach to container {} - it is not running (status: {:?})", container_id, status);
                    Err(format!("Container is not running (status: {:?})", status).into())
                }
            }
        } else {
            warn!("Cannot attach to session {} - no container ID found", session_id);
            Err("No container associated with this session".into())
        }
    }
    

    /// Kill the container for a session (force stop and cleanup)
    pub async fn kill_container(&mut self, session_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        use crate::docker::ContainerManager;
        
        // Find the session to get container ID
        let container_id = self.workspaces
            .iter()
            .flat_map(|w| &w.sessions)
            .find(|s| s.id == session_id)
            .and_then(|s| s.container_id.as_ref())
            .cloned();
        
        if let Some(container_id) = container_id {
            info!("Killing container {} for session {}", container_id, session_id);
            
            // Clear attached session if we're currently attached to this session
            if self.attached_session_id == Some(session_id) {
                self.attached_session_id = None;
                self.current_view = crate::app::state::View::SessionList;
                self.ui_needs_refresh = true;
            }
            
            let container_manager = ContainerManager::new().await?;
            
            // Force stop the container
            if let Some(mut session_container) = self.find_session_container_mut(session_id) {
                if let Err(e) = container_manager.stop_container(&mut session_container).await {
                    warn!("Failed to stop container gracefully: {}", e);
                }
                
                // Force remove the container
                if let Err(e) = container_manager.remove_container(&mut session_container).await {
                    error!("Failed to remove container: {}", e);
                    return Err(format!("Failed to remove container: {}", e).into());
                }
                
                info!("Successfully killed and removed container {} for session {}", container_id, session_id);
            }
            
            Ok(())
        } else {
            warn!("Cannot kill container for session {} - no container ID found", session_id);
            Err("No container associated with this session".into())
        }
    }

    /// Helper method to find a session container by session ID
    fn find_session_container_mut(&mut self, session_id: Uuid) -> Option<&mut crate::docker::SessionContainer> {
        // This is a simplified approach - in a real implementation you'd need to track
        // SessionContainer objects separately or modify the Session model to include them
        None // Placeholder - would need container tracking
    }

    /// Fetch container logs for a session
    pub async fn fetch_container_logs(&mut self, session_id: Uuid) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        use crate::docker::ContainerManager;
        
        // Find the session to get container ID
        let container_id = self.workspaces
            .iter()
            .flat_map(|w| &w.sessions)
            .find(|s| s.id == session_id)
            .and_then(|s| s.container_id.as_ref())
            .cloned();
        
        if let Some(container_id) = container_id {
            let container_manager = ContainerManager::new().await?;
            let logs = container_manager.get_container_logs(&container_id, Some(50)).await?;
            
            // Update the logs cache
            self.logs.insert(session_id, logs.clone());
            
            Ok(logs)
        } else {
            // No container ID - return session creation logs if available
            Ok(self.logs.get(&session_id).cloned().unwrap_or_else(|| {
                vec!["No container associated with this session".to_string()]
            }))
        }
    }

    pub async fn new_session_in_current_dir(&mut self) {
        use crate::git::WorkspaceScanner;
        use std::env;
        
        info!("Starting new session in current directory");
        
        // Check if current directory is a git repository
        let current_dir = match env::current_dir() {
            Ok(dir) => {
                info!("Current directory: {:?}", dir);
                dir
            }
            Err(e) => {
                warn!("Failed to get current directory: {}", e);
                return;
            }
        };
        
        match WorkspaceScanner::validate_workspace(&current_dir) {
            Ok(true) => {
                info!("Current directory is a valid git repository: {:?}", current_dir);
            }
            Ok(false) => {
                warn!("Current directory is not a git repository: {:?}", current_dir);
                info!("Falling back to workspace search");
                // Fall back to workspace search since current directory is not a git repository
                self.start_workspace_search().await;
                return;
            }
            Err(e) => {
                error!("Failed to validate workspace: {}", e);
                info!("Falling back to workspace search due to validation error");
                // Fall back to workspace search on validation error
                self.start_workspace_search().await;
                return;
            }
        }
        
        // Generate branch name with UUID
        let branch_base = format!("claude/{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("session"));
        
        // Create new session state for current directory
        self.new_session_state = Some(NewSessionState {
            available_repos: vec![current_dir.clone()],
            filtered_repos: vec![(0, current_dir)],
            selected_repo_index: Some(0),
            branch_name: branch_base.clone(),
            step: NewSessionStep::InputBranch,  // Skip repo selection
            filter_text: String::new(),
            is_current_dir_mode: true,
        });
        
        self.current_view = View::NewSession;
        
        info!("Successfully created new session state with branch: {}", branch_base);
    }
    
    pub async fn start_workspace_search(&mut self) {
        info!("Starting workspace search from NonGitNotification view");
        
        // Always transition to SessionList first to get out of NonGitNotification
        self.current_view = View::SessionList;
        
        match SessionLoader::new().await {
            Ok(loader) => {
                match loader.get_available_repositories().await {
                    Ok(repos) => {
                        if repos.is_empty() {
                            warn!("No repositories found in default search paths");
                            // Even with no repos, show the search interface with empty list
                            // User can type to search or we can show helpful message
                            info!("Showing empty search interface - user can type to add paths");
                        }
                        
                        // Generate branch name with UUID
                        let branch_base = format!("claude/{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("session"));
                        
                        // Initialize filtered repos with all repos (even if empty)
                        let filtered_repos: Vec<(usize, std::path::PathBuf)> = 
                            repos.iter().enumerate().map(|(idx, path)| (idx, path.clone())).collect();
                        
                        // Check if user has already cancelled (e.g., pressed escape while loading)
                        if self.async_operation_cancelled {
                            info!("Operation was cancelled by user");
                            return;
                        }
                        
                        let has_repos = !filtered_repos.is_empty();
                        self.new_session_state = Some(NewSessionState {
                            available_repos: repos,
                            filtered_repos,
                            selected_repo_index: if has_repos { Some(0) } else { None },
                            branch_name: branch_base,
                            step: NewSessionStep::SelectRepo,
                            filter_text: String::new(),
                            is_current_dir_mode: false,
                        });
                        
                        self.current_view = View::SearchWorkspace;
                        info!("Successfully transitioned to SearchWorkspace view");
                    }
                    Err(e) => {
                        warn!("Failed to load repositories: {}", e);
                        // Still transition to search view with empty state
                        self.new_session_state = Some(NewSessionState {
                            available_repos: vec![],
                            filtered_repos: vec![],
                            selected_repo_index: None,
                            branch_name: format!("claude/{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("session")),
                            step: NewSessionStep::SelectRepo,
                            filter_text: String::new(),
                            is_current_dir_mode: false,
                        });
                        self.current_view = View::SearchWorkspace;
                        info!("Transitioned to SearchWorkspace view with empty state due to error");
                    }
                }
            }
            Err(e) => {
                warn!("Failed to create session loader: {}", e);
                // Still transition to search view with empty state
                self.new_session_state = Some(NewSessionState {
                    available_repos: vec![],
                    filtered_repos: vec![],
                    selected_repo_index: None,
                    branch_name: format!("claude/{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("session")),
                    step: NewSessionStep::SelectRepo,
                    filter_text: String::new(),
                    is_current_dir_mode: false,
                });
                self.current_view = View::SearchWorkspace;
                info!("Transitioned to SearchWorkspace view with empty state due to loader error");
            }
        }
    }

    pub async fn start_new_session(&mut self) {
        info!("Starting new session creation");
        
        // Get available repositories
        match SessionLoader::new().await {
            Ok(loader) => {
                match loader.get_available_repositories().await {
                    Ok(repos) => {
                        let has_repos = !repos.is_empty();
                        let filtered_repos: Vec<(usize, std::path::PathBuf)> = 
                            repos.iter().enumerate().map(|(idx, path)| (idx, path.clone())).collect();
                        
                        self.new_session_state = Some(NewSessionState {
                            available_repos: repos,
                            filtered_repos,
                            selected_repo_index: if has_repos { Some(0) } else { None },
                            branch_name: String::new(),
                            step: NewSessionStep::SelectRepo,
                            filter_text: String::new(),
                            is_current_dir_mode: false,
                        });
                        self.current_view = View::NewSession;
                    }
                    Err(e) => {
                        warn!("Failed to get available repositories: {}", e);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to create session loader: {}", e);
            }
        }
    }

    pub fn cancel_new_session(&mut self) {
        self.new_session_state = None;
        self.current_view = View::SessionList;
        // Also clear any pending async actions to prevent race conditions
        self.pending_async_action = None;
        // Set cancellation flag to prevent race conditions
        self.async_operation_cancelled = true;
    }

    pub fn new_session_next_repo(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if !state.filtered_repos.is_empty() {
                let current = state.selected_repo_index.unwrap_or(0);
                state.selected_repo_index = Some((current + 1) % state.filtered_repos.len());
            }
        }
    }

    pub fn new_session_prev_repo(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if !state.filtered_repos.is_empty() {
                let current = state.selected_repo_index.unwrap_or(0);
                state.selected_repo_index = Some(
                    if current == 0 {
                        state.filtered_repos.len() - 1
                    } else {
                        current - 1
                    }
                );
            }
        }
    }

    pub fn new_session_confirm_repo(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.selected_repo_index.is_some() {
                state.step = NewSessionStep::InputBranch;
                let uuid_str = uuid::Uuid::new_v4().to_string();
                state.branch_name = format!("claude-session-{}", &uuid_str[..8]);
            }
        }
    }

    pub fn new_session_update_branch(&mut self, ch: char) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputBranch {
                state.branch_name.push(ch);
            }
        }
    }

    pub fn new_session_backspace(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputBranch {
                state.branch_name.pop();
            }
        }
    }

    pub async fn new_session_create(&mut self) {
        let (repo_path, branch_name, session_id) = {
            if let Some(ref mut state) = self.new_session_state {
                if state.step == NewSessionStep::InputBranch {
                    if let Some(repo_index) = state.selected_repo_index {
                        if let Some((_, repo_path)) = state.filtered_repos.get(repo_index) {
                            state.step = NewSessionStep::Creating;
                            let session_id = uuid::Uuid::new_v4();
                            (repo_path.clone(), state.branch_name.clone(), session_id)
                        } else {
                            return;
                        }
                    } else {
                        return;
                    }
                } else {
                    return;
                }
            } else {
                return;
            }
        };
        
        // Create the session with log streaming
        match self.create_session_with_logs(&repo_path, &branch_name, session_id).await {
            Ok(()) => {
                info!("Session created successfully");
                // Reload workspaces BEFORE switching view to ensure UI shows new session immediately
                self.load_real_workspaces().await;
                // Force UI refresh to show new session immediately
                self.ui_needs_refresh = true;
                self.cancel_new_session();
            }
            Err(e) => {
                warn!("Failed to create session: {}", e);
                self.cancel_new_session();
            }
        }
    }

    async fn create_session_with_logs(&mut self, repo_path: &std::path::Path, branch_name: &str, session_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        use crate::docker::session_lifecycle::{SessionLifecycleManager, SessionRequest};
        
        // Create a channel for build logs
        let (log_sender, mut log_receiver) = mpsc::unbounded_channel::<String>();
        
        // Initialize logs for this session
        self.logs.insert(session_id, vec!["Starting session creation...".to_string()]);
        
        // Create a shared vector for logs
        let session_logs = Arc::new(Mutex::new(Vec::new()));
        let logs_clone = session_logs.clone();
        
        // Spawn a task to collect logs
        let session_id_clone = session_id;
        tokio::spawn(async move {
            while let Some(log_message) = log_receiver.recv().await {
                if let Ok(mut logs) = logs_clone.lock() {
                    logs.push(log_message.clone());
                }
                info!("Build log for session {}: {}", session_id_clone, log_message);
            }
        });
        
        let workspace_name = repo_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        let request = SessionRequest {
            session_id,
            workspace_name,
            workspace_path: repo_path.to_path_buf(),
            branch_name: branch_name.to_string(),
            base_branch: None,
            container_config: None,
        };
        
        // Add initial log message
        if let Some(session_logs) = self.logs.get_mut(&session_id) {
            session_logs.push("Creating worktree...".to_string());
        }
        
        let mut manager = SessionLifecycleManager::new().await?;
        
        // Pass the log sender to the session lifecycle manager
        let result = manager.create_session_with_logs(request, Some(log_sender)).await;
        
        // Wait a moment for logs to be collected
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Transfer collected logs to our main logs HashMap
        if let Ok(collected_logs) = session_logs.lock() {
            if let Some(logs) = self.logs.get_mut(&session_id) {
                logs.extend(collected_logs.clone());
            }
        }
        
        // Add completion log based on result
        if let Some(logs) = self.logs.get_mut(&session_id) {
            match &result {
                Ok(_) => logs.push("Session created successfully!".to_string()),
                Err(e) => logs.push(format!("Session creation failed: {}", e)),
            }
        }
        
        result.map(|_| ())?;
        Ok(())
    }

    async fn create_session_internal(&self, repo_path: &std::path::Path, branch_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        use crate::docker::session_lifecycle::{SessionLifecycleManager, SessionRequest};
        
        let session_id = uuid::Uuid::new_v4();
        let workspace_name = repo_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        let request = SessionRequest {
            session_id,
            workspace_name,
            workspace_path: repo_path.to_path_buf(),
            branch_name: branch_name.to_string(),
            base_branch: None,
            container_config: None,
        };
        
        let mut manager = SessionLifecycleManager::new().await?;
        manager.create_session(request, None).await?;
        
        Ok(())
    }

    async fn delete_session(&mut self, session_id: Uuid) -> anyhow::Result<()> {
        use crate::docker::SessionLifecycleManager;
        use crate::git::WorktreeManager;
        
        info!("Deleting session: {}", session_id);
        
        // Log workspace count before deletion
        let workspace_count_before = self.workspaces.len();
        let session_count_before: usize = self.workspaces.iter().map(|w| w.sessions.len()).sum();
        info!("Before deletion: {} workspaces, {} sessions", workspace_count_before, session_count_before);
        
        // Create session lifecycle manager
        let mut manager = SessionLifecycleManager::new().await?;
        
        // Try to remove the session through lifecycle manager first
        match manager.remove_session(session_id).await {
            Ok(_) => {
                info!("Session removed through lifecycle manager");
            }
            Err(e) => {
                warn!("Session not found in lifecycle manager: {}", e);
                info!("Attempting to remove orphaned worktree directly");
                
                // If session not found in lifecycle manager, it's likely an orphaned worktree
                // Remove the worktree directly
                let worktree_manager = WorktreeManager::new()?;
                if let Err(worktree_err) = worktree_manager.remove_worktree(session_id) {
                    warn!("Failed to remove worktree: {}", worktree_err);
                } else {
                    info!("Successfully removed orphaned worktree");
                }
            }
        }
        
        // Reload workspaces to ensure UI reflects the actual state
        self.load_real_workspaces().await;
        // Force UI refresh to show updated session list immediately
        self.ui_needs_refresh = true;
        
        // Log workspace count after deletion
        let workspace_count_after = self.workspaces.len();
        let session_count_after: usize = self.workspaces.iter().map(|w| w.sessions.len()).sum();
        info!("After deletion: {} workspaces, {} sessions", workspace_count_after, session_count_after);
        
        info!("Successfully deleted session: {}", session_id);
        Ok(())
    }

    pub async fn process_async_action(&mut self) -> anyhow::Result<()> {
        if let Some(action) = self.pending_async_action.take() {
            match action {
                AsyncAction::StartNewSession => {
                    self.start_new_session().await;
                }
                AsyncAction::StartWorkspaceSearch => {
                    // Add timeout to prevent hanging
                    use tokio::time::{timeout, Duration};
                    match timeout(Duration::from_secs(10), self.start_workspace_search()).await {
                        Ok(_) => {}
                        Err(_) => {
                            warn!("Workspace search timed out after 10 seconds");
                            // Return to safe state
                            self.new_session_state = None;
                            self.current_view = View::SessionList;
                            return Err(anyhow::anyhow!("Workspace search timed out"));
                        }
                    }
                }
                AsyncAction::NewSessionInCurrentDir => {
                    self.new_session_in_current_dir().await;
                }
                AsyncAction::CreateNewSession => {
                    self.new_session_create().await;
                }
                AsyncAction::DeleteSession(session_id) => {
                    if let Err(e) = self.delete_session(session_id).await {
                        error!("Failed to delete session {}: {}", session_id, e);
                    }
                }
                AsyncAction::RefreshWorkspaces => {
                    info!("Manual refresh triggered");
                    // Reload workspace data and force UI refresh
                    self.load_real_workspaces().await;
                    self.ui_needs_refresh = true;
                }
                AsyncAction::FetchContainerLogs(session_id) => {
                    info!("Fetching container logs for session {}", session_id);
                    if let Err(e) = self.fetch_container_logs(session_id).await {
                        warn!("Failed to fetch container logs for session {}: {}", session_id, e);
                    }
                    self.ui_needs_refresh = true;
                }
                AsyncAction::AttachToContainer(session_id) => {
                    info!("Attaching to container for session {}", session_id);
                    if let Err(e) = self.attach_to_container(session_id).await {
                        error!("Failed to attach to container for session {}: {}", session_id, e);
                    }
                    self.ui_needs_refresh = true;
                }
                AsyncAction::KillContainer(session_id) => {
                    info!("Killing container for session {}", session_id);
                    if let Err(e) = self.kill_container(session_id).await {
                        error!("Failed to kill container for session {}: {}", session_id, e);
                    }
                    self.ui_needs_refresh = true;
                }
                AsyncAction::AuthSetupOAuth => {
                    info!("Starting OAuth authentication setup");
                    if let Err(e) = self.run_oauth_setup().await {
                        error!("Failed to setup OAuth authentication: {}", e);
                        if let Some(ref mut auth_state) = self.auth_setup_state {
                            auth_state.error_message = Some(format!("OAuth setup failed: {}", e));
                            auth_state.is_processing = false;
                        }
                    }
                }
                AsyncAction::AuthSetupApiKey => {
                    info!("Saving API key authentication");
                    if let Err(e) = self.save_api_key().await {
                        error!("Failed to save API key: {}", e);
                        if let Some(ref mut auth_state) = self.auth_setup_state {
                            auth_state.error_message = Some(format!("Failed to save API key: {}", e));
                            auth_state.is_processing = false;
                        }
                    }
                }
                AsyncAction::ReauthenticateCredentials => {
                    info!("Starting re-authentication process");
                    if let Err(e) = self.handle_reauthenticate().await {
                        error!("Failed to re-authenticate: {}", e);
                    }
                }
            }
        }
        Ok(())
    }
    
    /// Run OAuth authentication setup
    async fn run_oauth_setup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use crossterm::{
            terminal::{disable_raw_mode, LeaveAlternateScreen},
            execute,
        };
        
        // Create auth directory
        let home_dir = dirs::home_dir()
            .ok_or("Could not determine home directory")?;
        let auth_dir = home_dir.join(".claude-in-a-box/auth");
        
        info!("Creating auth directory: {}", auth_dir.display());
        std::fs::create_dir_all(&auth_dir)?;
        
        // Update UI state to show we're starting
        if let Some(ref mut auth_state) = self.auth_setup_state {
            auth_state.is_processing = true;
            auth_state.error_message = Some("Preparing authentication setup...".to_string());
        }
        
        // First check if Docker is available
        if !self.is_docker_available().await {
            warn!("Docker is not available or not running");
            if let Some(ref mut auth_state) = self.auth_setup_state {
                auth_state.error_message = Some(
                    "❌ Docker is not available\n\n\
                     Please start Docker and try again.".to_string()
                );
                auth_state.is_processing = false;
            }
            return Err("Docker not available".into());
        }
        
        // Check if image exists
        let image_name = "claude-box:claude-dev";
        let image_check = std::process::Command::new("docker")
            .args(["image", "inspect", image_name])
            .output()?;
        
        if !image_check.status.success() {
            info!("Building claude-dev image...");
            let build_status = std::process::Command::new("docker")
                .args(["build", "-t", image_name, "docker/claude-dev"])
                .status()?;
            
            if !build_status.success() {
                if let Some(ref mut auth_state) = self.auth_setup_state {
                    auth_state.error_message = Some(
                        "❌ Failed to build claude-dev image\n\n\
                         Please check Docker and try again.".to_string()
                    );
                    auth_state.is_processing = false;
                }
                return Err("Failed to build image".into());
            }
        }
        
        // Temporarily exit TUI to run interactive container
        info!("Exiting TUI to run interactive authentication");
        
        // Disable raw mode and restore terminal
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
        
        println!("\n🔐 Claude Authentication Setup\n");
        println!("This will guide you through the OAuth authentication process.");
        println!("You'll be prompted to open a URL in your browser to complete authentication.\n");
        
        // Run the auth container interactively 
        // Use inherit for stdin/stdout/stderr to ensure proper TTY forwarding
        let status = std::process::Command::new("docker")
            .args([
                "run",
                "--rm",
                "-it",
                "-v",
                &format!("{}:/home/claude-user/.claude", auth_dir.display()),
                "-e",
                "PATH=/home/claude-user/.npm-global/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                "-e",
                "HOME=/home/claude-user",
                "-e",
                "AUTH_METHOD=oauth",  // Specify OAuth method
                "-w",
                "/home/claude-user",
                "--user",
                "claude-user",
                "--entrypoint",
                "bash",
                image_name,
                "-c",
                "/app/scripts/auth-setup.sh",
            ])
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()?;
        
        // Check if authentication was successful
        let credentials_path = auth_dir.join(".credentials.json");
        let success = status.success() && credentials_path.exists() && credentials_path.metadata()?.len() > 0;
        
        if success {
            println!("\n✅ Authentication successful!");
            println!("Press Enter to continue...");
            let _ = std::io::stdin().read_line(&mut String::new());
            
            // Success - transition to main view
            self.auth_setup_state = None;
            self.current_view = View::SessionList;
            self.check_current_directory_status();
            self.pending_async_action = Some(AsyncAction::RefreshWorkspaces);
        } else {
            println!("\n❌ Authentication failed!");
            println!("Press Enter to return to the authentication menu...");
            let _ = std::io::stdin().read_line(&mut String::new());
            
            if let Some(ref mut auth_state) = self.auth_setup_state {
                auth_state.error_message = Some(
                    "❌ Authentication failed\n\n\
                     Please try again or use API Key method.".to_string()
                );
                auth_state.is_processing = false;
            }
        }
        
        // Re-enable raw mode and return to TUI
        use crossterm::{
            terminal::{enable_raw_mode, EnterAlternateScreen},
        };
        let _ = enable_raw_mode();
        let _ = execute!(std::io::stdout(), EnterAlternateScreen);
        
        // Force UI refresh
        self.ui_needs_refresh = true;
        
        Ok(())
    }
    
    /// Check if Docker is available and running
    async fn is_docker_available(&self) -> bool {
        // Try to run a simple docker command to check if Docker is available
        match std::process::Command::new("docker")
            .args(["version", "--format", "{{.Server.Version}}"])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout);
                    info!("Docker is available, version: {}", version.trim());
                    true
                } else {
                    let error = String::from_utf8_lossy(&output.stderr);
                    warn!("Docker command failed: {}", error);
                    false
                }
            }
            Err(e) => {
                warn!("Docker not found or not accessible: {}", e);
                false
            }
        }
    }
    
    
    /// Save API key authentication
    async fn save_api_key(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let api_key = match &self.auth_setup_state {
            Some(auth_state) => auth_state.api_key_input.clone(),
            None => return Err("No API key to save".into()),
        };
        
        // Validate API key format
        if !api_key.starts_with("sk-") || api_key.len() < 20 {
            return Err("Invalid API key format".into());
        }
        
        // Create .env file in claude-in-a-box directory
        let home_dir = dirs::home_dir()
            .ok_or("Could not determine home directory")?;
        let claude_box_dir = home_dir.join(".claude-in-a-box");
        std::fs::create_dir_all(&claude_box_dir)?;
        
        let env_path = claude_box_dir.join(".env");
        std::fs::write(&env_path, format!("ANTHROPIC_API_KEY={}\n", api_key))?;
        
        info!("API key saved to {:?}", env_path);
        
        // Success - transition to main view
        self.auth_setup_state = None;
        self.current_view = View::SessionList;
        self.check_current_directory_status();
        self.pending_async_action = Some(AsyncAction::RefreshWorkspaces);
        
        Ok(())
    }
    
    /// Handle re-authentication of Claude credentials
    async fn handle_reauthenticate(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Check if any sessions are currently running
        let running_session_count = self.workspaces.iter()
            .map(|w| w.running_sessions().len())
            .sum::<usize>();
            
        if running_session_count > 0 {
            warn!("Found {} running sessions - re-authentication will affect them", running_session_count);
            
            // For now, we'll show an error and require manual session cleanup
            // TODO: Add confirmation dialog with option to stop sessions automatically
            if let Some(ref mut auth_state) = self.auth_setup_state {
                auth_state.error_message = Some(format!(
                    "❌ Cannot re-authenticate with {} running sessions\n\n\
                     Running sessions use the current credentials.\n\
                     Please stop all sessions before re-authenticating.\n\n\
                     Use 'd' to delete sessions or wait for them to complete.",
                    running_session_count
                ));
                auth_state.is_processing = false;
            } else {
                // Create auth state to show the error
                self.auth_setup_state = Some(AuthSetupState {
                    selected_method: AuthMethod::OAuth,
                    api_key_input: String::new(),
                    is_processing: false,
                    show_cursor: false,
                    error_message: Some(format!(
                        "❌ Cannot re-authenticate with {} running sessions\n\n\
                         Running sessions use the current credentials.\n\
                         Please stop all sessions before re-authenticating.\n\n\
                         Use 'd' to delete sessions or wait for them to complete.",
                        running_session_count
                    )),
                });
                self.current_view = View::AuthSetup;
            }
            return Ok(());
        }
        
        // No running sessions - safe to proceed with re-authentication
        info!("No running sessions found - proceeding with re-authentication");
        
        // Create backup of existing credentials
        let home_dir = dirs::home_dir()
            .ok_or("Could not determine home directory")?;
        let auth_dir = home_dir.join(".claude-in-a-box/auth");
        
        let credentials_path = auth_dir.join(".credentials.json");
        let claude_json_path = auth_dir.join(".claude.json");
        let backup_suffix = format!(".backup-{}", chrono::Utc::now().timestamp());
        
        // Create backups if files exist
        if credentials_path.exists() {
            let backup_path = credentials_path.with_extension(&format!("json{}", backup_suffix));
            std::fs::copy(&credentials_path, &backup_path)?;
            info!("Backed up credentials to {:?}", backup_path);
        }
        
        if claude_json_path.exists() {
            let backup_path = claude_json_path.with_extension(&format!("json{}", backup_suffix));
            std::fs::copy(&claude_json_path, &backup_path)?;
            info!("Backed up claude.json to {:?}", backup_path);
        }
        
        // Remove existing credentials to trigger re-authentication
        if credentials_path.exists() {
            std::fs::remove_file(&credentials_path)?;
            info!("Removed existing credentials");
        }
        
        if claude_json_path.exists() {
            std::fs::remove_file(&claude_json_path)?;
            info!("Removed existing claude.json");
        }
        
        // Initialize auth setup state and switch to auth view
        self.auth_setup_state = Some(AuthSetupState {
            selected_method: AuthMethod::OAuth, // Default to OAuth
            api_key_input: String::new(),
            is_processing: false,
            show_cursor: false,
            error_message: Some("🔄 Previous credentials cleared - please authenticate again".to_string()),
        });
        self.current_view = View::AuthSetup;
        
        info!("Re-authentication initiated - switched to auth setup view");
        Ok(())
    }
}

pub struct App {
    pub state: AppState,
}

impl App {
    pub fn new() -> Self {
        Self {
            state: AppState::new(),
        }
    }

    pub async fn init(&mut self) {
        // Check if this is first time setup
        if AppState::is_first_time_setup() {
            self.state.current_view = View::AuthSetup;
            self.state.auth_setup_state = Some(AuthSetupState {
                selected_method: AuthMethod::OAuth,
                api_key_input: String::new(),
                is_processing: false,
                error_message: None,
                show_cursor: false,
            });
        } else {
            self.state.check_current_directory_status();
            self.state.load_real_workspaces().await;
        }
    }

    pub async fn tick(&mut self) -> anyhow::Result<()> {
        // Process any pending async actions
        match self.state.process_async_action().await {
            Ok(()) => {},
            Err(e) => {
                warn!("Error processing async action: {}", e);
                // Return to safe state if there was an error
                self.state.new_session_state = None;
                self.state.current_view = View::SessionList;
                self.state.pending_async_action = None;
            }
        }
        
        
        // Update logic for the app (e.g., refresh container status)
        Ok(())
    }

    /// Check if UI needs immediate refresh and clear the flag
    pub fn needs_ui_refresh(&mut self) -> bool {
        if self.state.ui_needs_refresh {
            self.state.ui_needs_refresh = false;
            true
        } else {
            false
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}