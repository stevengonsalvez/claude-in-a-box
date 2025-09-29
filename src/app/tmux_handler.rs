// ABOUTME: Handles tmux session attachment and detachment from the TUI
// Provides methods to attach to tmux sessions and manage terminal state

use crate::models::SessionStatus;
use std::process::{Command, Stdio};
use uuid::Uuid;
use crossterm::{terminal::{disable_raw_mode, enable_raw_mode}, execute};
use std::io::{self, stdout};

impl super::AppState {
    /// Attach to a tmux session
    pub fn attach_to_session(&mut self, session_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        // Try to find session in workspaces first
        let session_info = self.workspaces
            .iter()
            .flat_map(|w| &w.sessions)
            .find(|s| s.id == session_id)
            .map(|s| (s.tmux_session_name.clone(), s.name.clone()));

        // If not found in workspaces, check SessionManager
        let session_info = session_info.or_else(|| {
            self.session_manager
                .get_session(session_id)
                .map(|s| (s.tmux_session_name.clone(), s.name.clone()))
        });

        if let Some((session_name, _display_name)) = session_info {
            // Update session status
            if let Some(session) = self.workspaces
                .iter_mut()
                .flat_map(|w| &mut w.sessions)
                .find(|s| s.id == session_id) {
                session.status = SessionStatus::Attached;
                session.last_accessed = chrono::Utc::now();
            }

            // Store the attached session ID
            self.attached_session_id = Some(session_id);
            
            // Disable raw mode for terminal
            disable_raw_mode()?;
            
            // Clear the screen
            execute!(stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?;
            
            // Attach to tmux session
            let status = Command::new("tmux")
                .args(&["attach-session", "-t", &session_name])
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()?;

            // Re-enable raw mode after tmux exits
            enable_raw_mode()?;
            
            // Update session status based on exit
            if let Some(session) = self.workspaces
                .iter_mut()
                .flat_map(|w| &mut w.sessions)
                .find(|s| s.id == session_id) {
                session.status = SessionStatus::Detached;
            }

            // Clear attached session ID
            self.attached_session_id = None;

            // Force UI refresh
            self.ui_needs_refresh = true;

            if !status.success() {
                return Err("Failed to attach to tmux session".into());
            }

            Ok(())
        } else {
            Err(format!("Session {} not found in workspaces or SessionManager", session_id).into())
        }
    }

    /// Get logs from a tmux session using pane capture
    pub async fn fetch_tmux_logs(&mut self, session_id: Uuid) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        // Try to find session in workspaces first
        let tmux_session_name = self.workspaces
            .iter()
            .flat_map(|w| &w.sessions)
            .find(|s| s.id == session_id)
            .map(|s| s.tmux_session_name.clone());

        // If not found in workspaces, check SessionManager
        let tmux_session_name = tmux_session_name.or_else(|| {
            self.session_manager
                .get_session(session_id)
                .map(|s| s.tmux_session_name.clone())
        });

        if let Some(session_name) = tmux_session_name {
            // Capture pane content
            let output = Command::new("tmux")
                .args(&["capture-pane", "-t", &session_name, "-p", "-S", "-2000"])
                .output()?;

            if output.status.success() {
                let content = String::from_utf8_lossy(&output.stdout);
                let lines: Vec<String> = content
                    .lines()
                    .map(|s| s.to_string())
                    .filter(|s| !s.trim().is_empty())
                    .collect();
                
                // Update the logs cache
                self.logs.insert(session_id, lines.clone());
                
                // Update session's recent_logs
                if let Some(session) = self.workspaces
                    .iter_mut()
                    .flat_map(|w| &mut w.sessions)
                    .find(|s| s.id == session_id) {
                    session.recent_logs = Some(lines.join("\n"));
                }
                
                Ok(lines)
            } else {
                Ok(vec!["Failed to capture tmux pane".to_string()])
            }
        } else {
            Ok(vec!["No tmux session found".to_string()])
        }
    }

    /// Check if a tmux session exists
    pub fn tmux_session_exists(&self, session_name: &str) -> bool {
        Command::new("tmux")
            .args(&["has-session", "-t", session_name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Kill a tmux session
    pub async fn kill_tmux_session(&mut self, session_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        // Try to find session in workspaces first
        let tmux_session_name = self.workspaces
            .iter()
            .flat_map(|w| &w.sessions)
            .find(|s| s.id == session_id)
            .map(|s| s.tmux_session_name.clone());

        // If not found in workspaces, check SessionManager
        let tmux_session_name = tmux_session_name.or_else(|| {
            self.session_manager
                .get_session(session_id)
                .map(|s| s.tmux_session_name.clone())
        });

        if let Some(session_name) = tmux_session_name {
            Command::new("tmux")
                .args(&["kill-session", "-t", &session_name])
                .status()?;

            // Update session status
            if let Some(session) = self.workspaces
                .iter_mut()
                .flat_map(|w| &mut w.sessions)
                .find(|s| s.id == session_id) {
                session.status = SessionStatus::Stopped;
            }
        }
        
        Ok(())
    }
}