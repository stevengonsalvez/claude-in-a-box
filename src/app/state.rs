// ABOUTME: Application state management and view switching logic

use crate::models::{Session, Workspace};
use crate::git::{WorkspaceScanner, RepositoryManager};
use std::collections::HashMap;
use uuid::Uuid;
use tracing::{warn, info};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    SessionList,
    Logs,
    Terminal,
    Help,
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
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        let mut state = Self::default();
        state.load_real_workspaces();
        state
    }

    pub fn load_real_workspaces(&mut self) {
        info!("Loading real workspaces from filesystem");
        
        let scanner = WorkspaceScanner::new();
        match scanner.scan() {
            Ok(scan_result) => {
                self.workspaces = scan_result.workspaces;
                
                // Update git status for each workspace
                for workspace in &mut self.workspaces {
                    if let Ok(repo_manager) = RepositoryManager::open(&workspace.path) {
                        match repo_manager.get_status() {
                            Ok(git_changes) => {
                                // Create a placeholder session for demonstration
                                // In future phases, real sessions will be loaded from persistence or containers
                                let mut session = Session::new(
                                    format!("{}-main", workspace.name),
                                    workspace.path.to_string_lossy().to_string()
                                );
                                session.git_changes = git_changes;
                                workspace.add_session(session);
                            }
                            Err(e) => {
                                warn!("Failed to get git status for workspace {}: {}", workspace.name, e);
                            }
                        }
                    }
                }
                
                if !scan_result.errors.is_empty() {
                    warn!("Workspace scan completed with {} errors", scan_result.errors.len());
                    for error in &scan_result.errors {
                        warn!("Scan error: {}", error);
                    }
                }
                
                info!("Loaded {} workspaces", self.workspaces.len());
            }
            Err(e) => {
                warn!("Failed to scan workspaces: {}", e);
                // Fall back to mock data if scan fails
                self.load_mock_data();
            }
        }
        
        // Set initial selection
        if !self.workspaces.is_empty() {
            self.selected_workspace_index = Some(0);
            if !self.workspaces[0].sessions.is_empty() {
                self.selected_session_index = Some(0);
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

    pub fn tick(&mut self) {
        // Update logic for the app (e.g., refresh container status)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}