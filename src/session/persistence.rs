// ABOUTME: Session persistence for tmux sessions across application restarts
// Saves and restores session metadata to survive application restarts

use crate::models::{Session, SessionStatus};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedSession {
    pub id: Uuid,
    pub name: String,
    pub workspace_path: String,
    pub branch_name: String,
    pub tmux_session_name: String,
    pub worktree_path: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_accessed: chrono::DateTime<chrono::Utc>,
}

pub struct SessionPersistence {
    storage_path: PathBuf,
}

impl SessionPersistence {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let home = std::env::var("HOME")?;
        let storage_path = PathBuf::from(home).join(".claude-box").join("sessions");
        
        // Ensure directory exists
        fs::create_dir_all(&storage_path)?;
        
        Ok(Self { storage_path })
    }

    /// Save a session to persistent storage
    pub fn save_session(&self, session: &Session) -> Result<(), Box<dyn std::error::Error>> {
        let persisted = PersistedSession {
            id: session.id,
            name: session.name.clone(),
            workspace_path: session.workspace_path.clone(),
            branch_name: session.branch_name.clone(),
            tmux_session_name: session.tmux_session_name.clone(),
            worktree_path: session.worktree_path.clone(),
            created_at: session.created_at,
            last_accessed: session.last_accessed,
        };

        let session_file = self.storage_path.join(format!("{}.json", session.id));
        let json = serde_json::to_string_pretty(&persisted)?;
        fs::write(session_file, json)?;
        
        Ok(())
    }

    /// Load all persisted sessions
    pub fn load_sessions(&self) -> Result<Vec<PersistedSession>, Box<dyn std::error::Error>> {
        let mut sessions = Vec::new();

        if !self.storage_path.exists() {
            return Ok(sessions);
        }

        for entry in fs::read_dir(&self.storage_path)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match fs::read_to_string(&path) {
                    Ok(content) => {
                        match serde_json::from_str::<PersistedSession>(&content) {
                            Ok(session) => sessions.push(session),
                            Err(e) => {
                                tracing::warn!("Failed to parse session file {:?}: {}", path, e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read session file {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(sessions)
    }

    /// Delete a persisted session
    pub fn delete_session(&self, session_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        let session_file = self.storage_path.join(format!("{}.json", session_id));
        if session_file.exists() {
            fs::remove_file(session_file)?;
        }
        Ok(())
    }

    /// Check if a tmux session is still alive
    pub fn is_tmux_session_alive(session_name: &str) -> bool {
        std::process::Command::new("tmux")
            .args(&["has-session", "-t", session_name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Restore sessions on application start
    pub fn restore_sessions(&self) -> Result<Vec<Session>, Box<dyn std::error::Error>> {
        let persisted = self.load_sessions()?;
        let mut restored = Vec::new();

        for p in persisted {
            // Check if tmux session still exists
            let status = if Self::is_tmux_session_alive(&p.tmux_session_name) {
                SessionStatus::Detached
            } else {
                SessionStatus::Stopped
            };

            let session = Session {
                id: p.id,
                name: p.name,
                workspace_path: p.workspace_path,
                worktree_path: p.worktree_path,
                branch_name: p.branch_name,
                tmux_session_name: p.tmux_session_name,
                tmux_pid: None,
                status,
                created_at: p.created_at,
                last_accessed: p.last_accessed,
                git_changes: Default::default(),
                recent_logs: None,
                skip_permissions: false,
                mode: crate::models::SessionMode::Interactive,
                boss_prompt: None,
                environment_vars: Default::default(),
            };

            restored.push(session);
        }

        Ok(restored)
    }
}