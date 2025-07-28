// ABOUTME: Session data model representing a Claude Code container instance with git worktree

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionMode {
    Interactive,  // Traditional interactive mode with shell access
    Boss,         // Non-interactive mode with direct prompt execution
}

impl Default for SessionMode {
    fn default() -> Self {
        SessionMode::Interactive
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Running,
    Stopped,
    Error(String),
}

impl SessionStatus {
    pub fn indicator(&self) -> &'static str {
        match self {
            SessionStatus::Running => "●",
            SessionStatus::Stopped => "⏸",
            SessionStatus::Error(_) => "✗",
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(self, SessionStatus::Running)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub name: String,
    pub workspace_path: String,
    pub branch_name: String,
    pub container_id: Option<String>,
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub git_changes: GitChanges,
    pub recent_logs: Option<String>,
    pub skip_permissions: bool,  // Whether to use --dangerously-skip-permissions flag
    pub mode: SessionMode,       // Interactive or Boss mode
    pub boss_prompt: Option<String>, // The prompt for boss mode execution
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
    pub fn new(name: String, workspace_path: String) -> Self {
        Self::new_with_options(name, workspace_path, false, SessionMode::Interactive, None)
    }

    pub fn new_with_options(name: String, workspace_path: String, skip_permissions: bool, mode: SessionMode, boss_prompt: Option<String>) -> Self {
        let now = Utc::now();
        let branch_name = format!("claude/{}", name.replace(' ', "-").to_lowercase());
        
        Self {
            id: Uuid::new_v4(),
            name,
            workspace_path,
            branch_name,
            container_id: None,
            status: SessionStatus::Stopped,
            created_at: now,
            last_accessed: now,
            git_changes: GitChanges::default(),
            recent_logs: None,
            skip_permissions,
            mode,
            boss_prompt,
        }
    }

    pub fn update_last_accessed(&mut self) {
        self.last_accessed = Utc::now();
    }

    pub fn set_status(&mut self, status: SessionStatus) {
        self.status = status;
        self.update_last_accessed();
    }

    pub fn set_container_id(&mut self, container_id: Option<String>) {
        self.container_id = container_id;
        self.update_last_accessed();
    }
}