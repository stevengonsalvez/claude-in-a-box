// ABOUTME: Session model for host-based tmux sessions
// Manages tmux sessions running directly on the host machine

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionMode {
    Interactive, // Traditional interactive mode with shell access
    Boss,        // Non-interactive mode with direct prompt execution
}

impl Default for SessionMode {
    fn default() -> Self {
        SessionMode::Interactive
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Created,
    Running,
    Attached,
    Detached,
    Stopped,
    Error(String),
}

impl SessionStatus {
    pub fn indicator(&self) -> &'static str {
        match self {
            SessionStatus::Created => "○",
            SessionStatus::Running => "●",
            SessionStatus::Attached => "▶",
            SessionStatus::Detached => "⏸",
            SessionStatus::Stopped => "□",
            SessionStatus::Error(_) => "✗",
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(self, SessionStatus::Running | SessionStatus::Attached | SessionStatus::Detached)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub name: String,
    pub workspace_path: String,
    pub worktree_path: String,  // Git worktree location
    pub branch_name: String,

    // Tmux session info (replaces container_id)
    pub tmux_session_name: String,
    pub tmux_pid: Option<u32>,

    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub git_changes: GitChanges,
    pub recent_logs: Option<String>,
    
    // Session configuration
    pub skip_permissions: bool, // Whether to use --dangerously-skip-permissions flag
    pub mode: SessionMode,      // Interactive or Boss mode
    pub boss_prompt: Option<String>, // The prompt for boss mode execution
    
    // Optional environment variables for the session
    pub environment_vars: HashMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitChanges {
    pub added: u32,
    pub modified: u32,
    pub deleted: u32,
}

impl GitChanges {
    pub fn total(&self) -> u32 {
        self.added + self.modified + self.deleted
    }

    pub fn format(&self) -> String {
        if self.total() == 0 {
            "No changes".to_string()
        } else {
            format!("+{} ~{} -{}", self.added, self.modified, self.deleted)
        }
    }
}

impl Session {
    /// Sanitize a name for use as a tmux session name
    /// Replaces all special characters that tmux doesn't handle well
    pub fn sanitize_tmux_name(name: &str) -> String {
        name.replace(' ', "_")
            .replace('.', "_")
            .replace('/', "_")
            .replace('\\', "_")
            .replace(':', "_")
            .replace(';', "_")
            .replace('|', "_")
            .replace('&', "_")
            .replace('(', "_")
            .replace(')', "_")
            .replace('<', "_")
            .replace('>', "_")
            .replace('"', "_")
            .replace('\'', "_")
    }

    pub fn new(name: String, workspace_path: String) -> Self {
        Self::new_with_options(name, workspace_path, false, SessionMode::Interactive, None)
    }

    pub fn new_with_options(
        name: String,
        workspace_path: String,
        skip_permissions: bool,
        mode: SessionMode,
        boss_prompt: Option<String>,
    ) -> Self {
        let now = Utc::now();
        let branch_name = format!("claude/{}", name.replace(' ', "-").to_lowercase());
        let tmux_session_name = format!("ciab_{}", Self::sanitize_tmux_name(&name));
        let worktree_path = format!("{}/.worktrees/{}", workspace_path, branch_name);

        Self {
            id: Uuid::new_v4(),
            name,
            workspace_path: workspace_path.clone(),
            worktree_path,
            branch_name,
            tmux_session_name,
            tmux_pid: None,
            status: SessionStatus::Created,
            created_at: now,
            last_accessed: now,
            git_changes: GitChanges::default(),
            recent_logs: None,
            skip_permissions,
            mode,
            boss_prompt,
            environment_vars: HashMap::new(),
        }
    }

    pub fn update_last_accessed(&mut self) {
        self.last_accessed = Utc::now();
    }

    pub fn set_status(&mut self, status: SessionStatus) {
        self.status = status;
        self.update_last_accessed();
    }

    pub fn set_tmux_pid(&mut self, pid: Option<u32>) {
        self.tmux_pid = pid;
        self.update_last_accessed();
    }
}