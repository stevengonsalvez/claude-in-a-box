// ABOUTME: Docker log streaming manager for real-time container log collection
// Streams logs from Docker containers to the live logs UI component

use crate::components::live_logs_stream::{LogEntry, LogEntryLevel};
use crate::docker::ContainerManager;
use anyhow::{Result, anyhow};
use bollard::container::{LogOutput, LogsOptions};
use futures_util::StreamExt;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Debug)]
pub struct DockerLogStreamingManager {
    container_manager: ContainerManager,
    streaming_tasks: HashMap<Uuid, StreamingTask>,
    log_sender: mpsc::UnboundedSender<(Uuid, LogEntry)>,
    session_modes: HashMap<Uuid, crate::models::SessionMode>, // Track session modes for proper parsing
}

#[derive(Debug)]
struct StreamingTask {
    container_id: String,
    _container_name: String,
    task_handle: JoinHandle<()>,
}

impl DockerLogStreamingManager {
    /// Create a new log streaming manager
    pub fn new(log_sender: mpsc::UnboundedSender<(Uuid, LogEntry)>) -> Result<Self> {
        Ok(Self {
            container_manager: ContainerManager::new_sync()?,
            streaming_tasks: HashMap::new(),
            log_sender,
            session_modes: HashMap::new(),
        })
    }

    /// Start streaming logs for a session's container
    pub async fn start_streaming(
        &mut self,
        session_id: Uuid,
        container_id: String,
        container_name: String,
        session_mode: crate::models::SessionMode,
    ) -> Result<()> {
        // Stop any existing streaming for this session
        self.stop_streaming(session_id).await?;

        info!(
            "Starting log streaming for session {} (container: {}) in {:?} mode",
            session_id, container_id, session_mode
        );

        // Store session mode for parsing
        self.session_modes.insert(session_id, session_mode.clone());

        let log_sender = self.log_sender.clone();
        let container_id_clone = container_id.clone();
        let container_name_clone = container_name.clone();
        let docker = self.container_manager.get_docker_client();

        // Spawn a task to stream logs
        let task_handle = tokio::spawn(async move {
            if let Err(e) = Self::stream_container_logs(
                docker,
                session_id,
                container_id_clone.clone(),
                container_name_clone.clone(),
                log_sender,
                session_mode,
            )
            .await
            {
                error!(
                    "Log streaming error for container {}: {}",
                    container_id_clone, e
                );
            }
        });

        self.streaming_tasks.insert(
            session_id,
            StreamingTask {
                container_id,
                _container_name: container_name,
                task_handle,
            },
        );

        Ok(())
    }

    /// Stop streaming logs for a session
    pub async fn stop_streaming(&mut self, session_id: Uuid) -> Result<()> {
        if let Some(task) = self.streaming_tasks.remove(&session_id) {
            info!(
                "Stopping log streaming for session {} (container: {})",
                session_id, task.container_id
            );
            task.task_handle.abort();
        }
        // Remove session mode tracking
        self.session_modes.remove(&session_id);
        Ok(())
    }

    /// Stop all log streaming
    pub async fn stop_all_streaming(&mut self) -> Result<()> {
        info!("Stopping all log streaming tasks");
        for (_, task) in self.streaming_tasks.drain() {
            task.task_handle.abort();
        }
        // Clear all session mode tracking
        self.session_modes.clear();
        Ok(())
    }

    /// Get active streaming sessions
    pub fn active_sessions(&self) -> Vec<Uuid> {
        self.streaming_tasks.keys().cloned().collect()
    }

    /// Check if streaming is active for a session
    pub fn is_streaming(&self, session_id: Uuid) -> bool {
        self.streaming_tasks.contains_key(&session_id)
    }

    /// Stream logs from a container
    async fn stream_container_logs(
        docker: bollard::Docker,
        session_id: Uuid,
        container_id: String,
        container_name: String,
        log_sender: mpsc::UnboundedSender<(Uuid, LogEntry)>,
        session_mode: crate::models::SessionMode,
    ) -> Result<()> {
        let options = LogsOptions::<String> {
            stdout: true,
            stderr: true,
            follow: true,
            timestamps: true,
            tail: "100".to_string(), // Start with last 100 lines
            ..Default::default()
        };

        debug!(
            "Starting log stream for container {} (session {})",
            container_id, session_id
        );

        let mut log_stream = docker.logs(&container_id, Some(options));

        // Send initial connection message
        let _ = log_sender.send((
            session_id,
            LogEntry::new(
                LogEntryLevel::Info,
                "system".to_string(),
                format!("📡 Connected to container logs: {}", container_name),
            )
            .with_session(session_id),
        ));

        while let Some(log_result) = log_stream.next().await {
            match log_result {
                Ok(log_output) => {
                    let log_entry = Self::parse_log_output(
                        log_output,
                        &container_name,
                        session_id,
                        &session_mode,
                    );

                    if let Err(e) = log_sender.send((session_id, log_entry)) {
                        warn!("Failed to send log entry: {}", e);
                        break; // Channel closed, stop streaming
                    }
                }
                Err(e) => {
                    error!("Error reading log stream: {}", e);
                    let _ = log_sender.send((
                        session_id,
                        LogEntry::new(
                            LogEntryLevel::Error,
                            "system".to_string(),
                            format!("❌ Log stream error: {}", e),
                        )
                        .with_session(session_id),
                    ));
                    break;
                }
            }
        }

        debug!(
            "Log stream ended for container {} (session {})",
            container_id, session_id
        );

        // Send disconnection message
        let _ = log_sender.send((
            session_id,
            LogEntry::new(
                LogEntryLevel::Info,
                "system".to_string(),
                format!("📡 Disconnected from container logs: {}", container_name),
            )
            .with_session(session_id),
        ));

        Ok(())
    }

    /// Parse Docker log output into a LogEntry
    fn parse_log_output(
        log_output: LogOutput,
        container_name: &str,
        session_id: Uuid,
        session_mode: &crate::models::SessionMode,
    ) -> LogEntry {
        let (message, is_stderr) = match log_output {
            LogOutput::StdOut { message } => (String::from_utf8_lossy(&message).to_string(), false),
            LogOutput::StdErr { message } => (String::from_utf8_lossy(&message).to_string(), true),
            LogOutput::Console { message } => {
                (String::from_utf8_lossy(&message).to_string(), false)
            }
            LogOutput::StdIn { message } => (String::from_utf8_lossy(&message).to_string(), false),
        };

        // Clean up the message (remove trailing newlines)
        let message = message.trim_end().to_string();

        // Use boss mode parsing if this is a boss mode session
        let is_boss_mode = matches!(session_mode, crate::models::SessionMode::Boss);

        if is_boss_mode {
            LogEntry::from_docker_log_with_mode(container_name, &message, Some(session_id), true)
        } else {
            // Determine log level based on content and stream type for interactive mode
            let level = if is_stderr {
                LogEntryLevel::Error
            } else {
                LogEntry::parse_level_from_message(&message)
            };

            LogEntry::new(level, container_name.to_string(), message).with_session(session_id)
        }
    }

    /// Start streaming logs for all active sessions
    pub async fn start_streaming_for_sessions(
        &mut self,
        sessions: &[(Uuid, String, String, crate::models::SessionMode)], // (session_id, container_id, container_name, session_mode)
    ) -> Result<()> {
        for (session_id, container_id, container_name, session_mode) in sessions {
            if let Err(e) = self
                .start_streaming(
                    *session_id,
                    container_id.clone(),
                    container_name.clone(),
                    session_mode.clone(),
                )
                .await
            {
                warn!(
                    "Failed to start log streaming for session {}: {}",
                    session_id, e
                );
            }
        }
        Ok(())
    }
}

impl Drop for DockerLogStreamingManager {
    fn drop(&mut self) {
        // Abort all streaming tasks when manager is dropped
        for (_, task) in self.streaming_tasks.drain() {
            task.task_handle.abort();
        }
    }
}

/// Log streaming coordinator for the application
#[derive(Debug)]
pub struct LogStreamingCoordinator {
    manager: Option<DockerLogStreamingManager>,
    log_receiver: mpsc::UnboundedReceiver<(Uuid, LogEntry)>,
}

impl LogStreamingCoordinator {
    /// Create a new coordinator with channels for log communication
    pub fn new() -> (Self, mpsc::UnboundedSender<(Uuid, LogEntry)>) {
        let (log_sender, log_receiver) = mpsc::unbounded_channel();

        (
            Self {
                manager: None,
                log_receiver,
            },
            log_sender,
        )
    }

    /// Initialize the streaming manager
    pub fn init_manager(
        &mut self,
        log_sender: mpsc::UnboundedSender<(Uuid, LogEntry)>,
    ) -> Result<()> {
        self.manager = Some(DockerLogStreamingManager::new(log_sender)?);
        Ok(())
    }

    /// Get the next log entry from any container (non-blocking)
    pub fn try_next_log(&mut self) -> Option<(Uuid, LogEntry)> {
        self.log_receiver.try_recv().ok()
    }

    /// Get the next log entry from any container (blocking)
    pub async fn next_log(&mut self) -> Option<(Uuid, LogEntry)> {
        self.log_receiver.recv().await
    }

    /// Start streaming for a session
    pub async fn start_streaming(
        &mut self,
        session_id: Uuid,
        container_id: String,
        container_name: String,
        session_mode: crate::models::SessionMode,
    ) -> Result<()> {
        if let Some(manager) = &mut self.manager {
            manager
                .start_streaming(session_id, container_id, container_name, session_mode)
                .await
        } else {
            Err(anyhow!("Log streaming manager not initialized"))
        }
    }

    /// Stop streaming for a session
    pub async fn stop_streaming(&mut self, session_id: Uuid) -> Result<()> {
        if let Some(manager) = &mut self.manager {
            manager.stop_streaming(session_id).await
        } else {
            Err(anyhow!("Log streaming manager not initialized"))
        }
    }

    /// Stop all streaming
    pub async fn stop_all(&mut self) -> Result<()> {
        if let Some(manager) = &mut self.manager {
            manager.stop_all_streaming().await
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_log_entry_parsing() {
        let container_name = "test-container";
        let session_id = Uuid::new_v4();

        // Test stdout parsing in interactive mode
        let stdout = LogOutput::StdOut {
            message: b"INFO: Test message\n".to_vec().into(),
        };
        let entry = DockerLogStreamingManager::parse_log_output(
            stdout,
            container_name,
            session_id,
            &crate::models::SessionMode::Interactive,
        );
        assert_eq!(entry.level, LogEntryLevel::Info);
        assert_eq!(entry.message, "INFO: Test message");

        // Test stderr parsing in interactive mode
        let stderr = LogOutput::StdErr {
            message: b"Error occurred\n".to_vec().into(),
        };
        let entry = DockerLogStreamingManager::parse_log_output(
            stderr,
            container_name,
            session_id,
            &crate::models::SessionMode::Interactive,
        );
        assert_eq!(entry.level, LogEntryLevel::Error);
        assert_eq!(entry.message, "Error occurred");

        // Test boss mode parsing (JSON fallback still works)
        let boss_stdout = LogOutput::StdOut {
            message: b"{\"type\": \"message\", \"content\": \"Hello from Claude!\"}\n"
                .to_vec()
                .into(),
        };
        let entry = DockerLogStreamingManager::parse_log_output(
            boss_stdout,
            container_name,
            session_id,
            &crate::models::SessionMode::Boss,
        );
        assert_eq!(entry.level, LogEntryLevel::Info);
        assert_eq!(entry.message, "🤖 Claude: Hello from Claude!");
        assert_eq!(entry.source, "claude-boss");
    }
}
