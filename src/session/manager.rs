// ABOUTME: Session lifecycle management for host tmux sessions
// Manages creation, attachment, and cleanup of tmux sessions

use crate::tmux::TmuxSession;
use crate::git::WorktreeManager;
use crate::models::{Session, SessionStatus};
use std::collections::HashMap;
use std::process::Command;
use uuid::Uuid;

pub struct SessionManager {
    sessions: HashMap<Uuid, Session>,
    tmux_sessions: HashMap<Uuid, TmuxSession>,
    worktree_manager: WorktreeManager,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            tmux_sessions: HashMap::new(),
            worktree_manager: WorktreeManager::new(),
        }
    }

    pub async fn create_session(
        &mut self,
        workspace_path: &str,
        branch_name: &str,
        session_name: &str,
    ) -> Result<Uuid, Box<dyn std::error::Error>> {
        let session_id = Uuid::new_v4();

        // Create git worktree
        let worktree_path = self.worktree_manager
            .create_worktree(workspace_path, branch_name, &session_name)?;

        // Optional environment variables - Claude CLI uses host config
        let mut env_vars = HashMap::new();
        env_vars.insert("CIAB_SESSION".to_string(), session_name.to_string());
        env_vars.insert("CIAB_WORKTREE".to_string(), worktree_path.clone());

        // Determine program to run (claude if available, else bash)
        let program = if Command::new("which")
            .arg("claude")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            "claude"
        } else {
            "/bin/bash -l"
        };

        // Create tmux session
        let tmux_session = TmuxSession::create(
            session_name,
            &worktree_path,
            program,
            &env_vars,
        ).await?;

        // Create session model
        let session = Session {
            id: session_id,
            name: session_name.to_string(),
            workspace_path: workspace_path.to_string(),
            worktree_path: worktree_path.clone(),
            branch_name: branch_name.to_string(),
            tmux_session_name: tmux_session.name.clone(),
            tmux_pid: None,
            status: SessionStatus::Running,
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            git_changes: Default::default(),
            recent_logs: None,
            environment_vars: env_vars,
            skip_permissions: false,
            mode: crate::models::SessionMode::Interactive,
            boss_prompt: None,
        };

        self.sessions.insert(session_id, session);
        self.tmux_sessions.insert(session_id, tmux_session);

        Ok(session_id)
    }

    pub async fn attach_session(&mut self, session_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        let tmux_session = self.tmux_sessions.get_mut(&session_id)
            .ok_or("Session not found")?;

        tmux_session.attach().await?;

        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.status = SessionStatus::Attached;
            session.last_accessed = chrono::Utc::now();
        }

        Ok(())
    }

    pub async fn detach_session(&mut self, session_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        let tmux_session = self.tmux_sessions.get_mut(&session_id)
            .ok_or("Session not found")?;

        tmux_session.detach().await?;

        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.status = SessionStatus::Detached;
        }

        Ok(())
    }

    pub async fn restore_sessions(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Find existing CIAB tmux sessions
        let existing_sessions = TmuxSession::list_sessions().await?;

        for session_name in existing_sessions {
            // Try to restore session metadata from disk or recreate
            println!("Found existing tmux session: {}", session_name);
            // TODO: Implement session restoration logic
        }

        Ok(())
    }

    pub async fn cleanup_session(&mut self, session_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        // Kill tmux session
        if let Some(mut tmux_session) = self.tmux_sessions.remove(&session_id) {
            tmux_session.kill().await?;
        }

        // Clean up worktree
        if let Some(session) = self.sessions.get(&session_id) {
            self.worktree_manager.remove_worktree(&session.worktree_path)?;
        }

        // Remove from sessions map
        self.sessions.remove(&session_id);

        Ok(())
    }

    pub fn get_sessions(&self) -> Vec<&Session> {
        self.sessions.values().collect()
    }

    pub fn get_session(&self, session_id: Uuid) -> Option<&Session> {
        self.sessions.get(&session_id)
    }

    pub fn get_session_mut(&mut self, session_id: Uuid) -> Option<&mut Session> {
        self.sessions.get_mut(&session_id)
    }

    pub fn get_tmux_session_mut(&mut self, session_id: Uuid) -> Option<&mut TmuxSession> {
        self.tmux_sessions.get_mut(&session_id)
    }

    pub fn get_all_tmux_sessions(&mut self) -> Vec<&mut TmuxSession> {
        self.tmux_sessions.values_mut().collect()
    }

    pub async fn capture_session_pane(&self, session_id: Uuid) -> Result<String, Box<dyn std::error::Error>> {
        let tmux_session = self.tmux_sessions.get(&session_id)
            .ok_or("Session not found")?;

        Ok(tmux_session.capture_pane().await?)
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}