// ABOUTME: Application state management and view switching logic

use crate::models::{Session, Workspace};
use crate::app::SessionLoader;
use std::collections::HashMap;
use uuid::Uuid;
use tracing::{warn, info};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    SessionList,
    Logs,
    Terminal,
    Help,
    NewSession,
    SearchWorkspace,
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
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
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
        }
    }

    pub fn toggle_help(&mut self) {
        self.help_visible = !self.help_visible;
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub async fn new_session_in_current_dir(&mut self) {
        use crate::git::WorkspaceScanner;
        use std::env;
        
        // Check if current directory is a git repository
        let current_dir = match env::current_dir() {
            Ok(dir) => dir,
            Err(e) => {
                warn!("Failed to get current directory: {}", e);
                return;
            }
        };
        
        if !WorkspaceScanner::validate_workspace(&current_dir).unwrap_or(false) {
            // Show error message - current directory is not a git repository
            warn!("Current directory is not a git repository");
            // TODO: Show error in UI
            return;
        }
        
        // Generate branch name with UUID
        let branch_base = format!("claude/{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("session"));
        
        // Create new session state for current directory
        self.new_session_state = Some(NewSessionState {
            available_repos: vec![current_dir.clone()],
            filtered_repos: vec![(0, current_dir)],
            selected_repo_index: Some(0),
            branch_name: branch_base,
            step: NewSessionStep::InputBranch,  // Skip repo selection
            filter_text: String::new(),
            is_current_dir_mode: true,
        });
        
        self.current_view = View::NewSession;
    }
    
    pub async fn start_workspace_search(&mut self) {
        match SessionLoader::new().await {
            Ok(loader) => {
                match loader.get_available_repositories().await {
                    Ok(repos) => {
                        if repos.is_empty() {
                            warn!("No repositories found");
                            return;
                        }
                        
                        // Generate branch name with UUID
                        let branch_base = format!("claude/{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("session"));
                        
                        // Initialize filtered repos with all repos
                        let filtered_repos: Vec<(usize, std::path::PathBuf)> = 
                            repos.iter().enumerate().map(|(idx, path)| (idx, path.clone())).collect();
                        
                        // Check if user has already cancelled (e.g., pressed escape while loading)
                        if self.async_operation_cancelled {
                            return;
                        }
                        
                        self.new_session_state = Some(NewSessionState {
                            available_repos: repos,
                            filtered_repos,
                            selected_repo_index: Some(0),
                            branch_name: branch_base,
                            step: NewSessionStep::SelectRepo,
                            filter_text: String::new(),
                            is_current_dir_mode: false,
                        });
                        
                        self.current_view = View::SearchWorkspace;
                    }
                    Err(e) => {
                        warn!("Failed to load repositories: {}", e);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to create session loader: {}", e);
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
        let (repo_path, branch_name) = {
            if let Some(ref mut state) = self.new_session_state {
                if state.step == NewSessionStep::InputBranch {
                    if let Some(repo_index) = state.selected_repo_index {
                        if let Some((_, repo_path)) = state.filtered_repos.get(repo_index) {
                            state.step = NewSessionStep::Creating;
                            (repo_path.clone(), state.branch_name.clone())
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
        
        // Create the session
        match self.create_session_internal(&repo_path, &branch_name).await {
            Ok(()) => {
                info!("Session created successfully");
                self.cancel_new_session();
                self.load_real_workspaces().await;
            }
            Err(e) => {
                warn!("Failed to create session: {}", e);
                self.cancel_new_session();
            }
        }
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
        manager.create_session(request).await?;
        
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
            }
        }
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
        self.state.load_real_workspaces().await;
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
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}