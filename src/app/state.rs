// ABOUTME: Application state management and view switching logic

use crate::app::SessionLoader;
use crate::claude::client::ClaudeChatManager;
use crate::claude::types::ClaudeStreamingEvent;
use crate::claude::{ClaudeApiClient, ClaudeMessage};
use crate::components::fuzzy_file_finder::FuzzyFileFinderState;
use crate::components::live_logs_stream::LogEntry;
use crate::docker::LogStreamingCoordinator;
use crate::models::{Session, Workspace};
use std::collections::HashMap;
use std::time::{Duration, Instant};

use chrono;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Text editor with cursor support for boss mode prompts
#[derive(Debug, Clone)]
pub struct TextEditor {
    lines: Vec<String>,
    cursor_line: usize,
    cursor_col: usize,
}

impl TextEditor {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
        }
    }

    pub fn from_string(text: &str) -> Self {
        let lines: Vec<String> = if text.is_empty() {
            vec![String::new()]
        } else {
            text.lines().map(|s| s.to_string()).collect()
        };

        Self {
            lines,
            cursor_line: 0,
            cursor_col: 0,
        }
    }

    pub fn to_string(&self) -> String {
        self.lines.join("\n")
    }

    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    pub fn insert_char(&mut self, ch: char) {
        if ch == '\n' {
            self.insert_newline();
        } else {
            let line = &mut self.lines[self.cursor_line];
            line.insert(self.cursor_col, ch);
            self.cursor_col += 1;
        }
    }

    pub fn insert_newline(&mut self) {
        let current_line = self.lines[self.cursor_line].clone();
        let (left, right) = current_line.split_at(self.cursor_col);

        self.lines[self.cursor_line] = left.to_string();
        self.lines.insert(self.cursor_line + 1, right.to_string());

        self.cursor_line += 1;
        self.cursor_col = 0;
    }

    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            // Delete character before cursor
            self.lines[self.cursor_line].remove(self.cursor_col - 1);
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            // Join with previous line
            let current_line = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            self.cursor_col = self.lines[self.cursor_line].len();
            self.lines[self.cursor_line].push_str(&current_line);
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.lines[self.cursor_line].len();
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_col < self.lines[self.cursor_line].len() {
            self.cursor_col += 1;
        } else if self.cursor_line < self.lines.len() - 1 {
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_cursor_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.cursor_col.min(self.lines[self.cursor_line].len());
        }
    }

    pub fn move_cursor_down(&mut self) {
        if self.cursor_line < self.lines.len() - 1 {
            self.cursor_line += 1;
            self.cursor_col = self.cursor_col.min(self.lines[self.cursor_line].len());
        }
    }

    pub fn move_to_line_start(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_to_line_end(&mut self) {
        self.cursor_col = self.lines[self.cursor_line].len();
    }

    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        let mut lines = text.lines();

        // Insert first line of text at current cursor position
        if let Some(first_line) = lines.next() {
            self.lines[self.cursor_line].insert_str(self.cursor_col, first_line);
            self.cursor_col += first_line.len();
        }

        // Insert newlines and subsequent lines
        for line in lines {
            self.insert_newline();
            self.lines[self.cursor_line].insert_str(self.cursor_col, line);
            self.cursor_col += line.len();
        }
    }

    pub fn get_cursor_position(&self) -> (usize, usize) {
        (self.cursor_line, self.cursor_col)
    }

    pub fn get_lines(&self) -> &Vec<String> {
        &self.lines
    }

    pub fn move_cursor_to_end(&mut self) {
        if !self.lines.is_empty() {
            self.cursor_line = self.lines.len() - 1;
            self.cursor_col = self.lines[self.cursor_line].len();
        }
    }

    pub fn set_cursor_position(&mut self, line: usize, col: usize) {
        if line < self.lines.len() {
            self.cursor_line = line;
            self.cursor_col = col.min(self.lines[line].len());
        }
    }

    // Word movement methods
    pub fn move_cursor_word_forward(&mut self) {
        let current_line = &self.lines[self.cursor_line];

        // If at end of line, move to next line
        if self.cursor_col >= current_line.len() {
            if self.cursor_line < self.lines.len() - 1 {
                self.cursor_line += 1;
                self.cursor_col = 0;
                // Find first non-whitespace character
                let next_line = &self.lines[self.cursor_line];
                while self.cursor_col < next_line.len()
                    && next_line.chars().nth(self.cursor_col).unwrap().is_whitespace()
                {
                    self.cursor_col += 1;
                }
            }
            return;
        }

        let chars: Vec<char> = current_line.chars().collect();
        let mut pos = self.cursor_col;

        // Skip current word
        while pos < chars.len()
            && !chars[pos].is_whitespace()
            && chars[pos] != '.'
            && chars[pos] != ','
        {
            pos += 1;
        }

        // Skip whitespace
        while pos < chars.len() && chars[pos].is_whitespace() {
            pos += 1;
        }

        self.cursor_col = pos;
    }

    pub fn move_cursor_word_backward(&mut self) {
        // If at beginning of line, move to end of previous line
        if self.cursor_col == 0 {
            if self.cursor_line > 0 {
                self.cursor_line -= 1;
                self.cursor_col = self.lines[self.cursor_line].len();
            }
            return;
        }

        let current_line = &self.lines[self.cursor_line];
        let chars: Vec<char> = current_line.chars().collect();
        let mut pos = self.cursor_col.saturating_sub(1);

        // Skip whitespace backwards
        while pos > 0 && chars[pos].is_whitespace() {
            pos = pos.saturating_sub(1);
        }

        // Skip word backwards
        while pos > 0 && !chars[pos].is_whitespace() && chars[pos] != '.' && chars[pos] != ',' {
            pos = pos.saturating_sub(1);
        }

        // If we stopped on whitespace or punctuation, move forward one
        if pos > 0 && (chars[pos].is_whitespace() || chars[pos] == '.' || chars[pos] == ',') {
            pos += 1;
        }

        self.cursor_col = pos;
    }

    // Word deletion methods
    pub fn delete_word_forward(&mut self) {
        let current_line_text = self.lines[self.cursor_line].clone();
        let chars: Vec<char> = current_line_text.chars().collect();
        let start_pos = self.cursor_col;

        if start_pos >= chars.len() {
            return;
        }

        let mut end_pos = start_pos;

        // Skip current word
        while end_pos < chars.len()
            && !chars[end_pos].is_whitespace()
            && chars[end_pos] != '.'
            && chars[end_pos] != ','
        {
            end_pos += 1;
        }

        // Skip following whitespace
        while end_pos < chars.len() && chars[end_pos].is_whitespace() {
            end_pos += 1;
        }

        // Remove the text
        let before: String = chars[..start_pos].iter().collect();
        let after: String = chars[end_pos..].iter().collect();
        self.lines[self.cursor_line] = format!("{}{}", before, after);
    }

    pub fn delete_word_backward(&mut self) {
        if self.cursor_col == 0 {
            return;
        }

        let current_line_text = self.lines[self.cursor_line].clone();
        let chars: Vec<char> = current_line_text.chars().collect();
        let end_pos = self.cursor_col;
        let mut start_pos = end_pos.saturating_sub(1);

        // Skip whitespace backwards
        while start_pos > 0 && chars[start_pos].is_whitespace() {
            start_pos = start_pos.saturating_sub(1);
        }

        // Skip word backwards
        while start_pos > 0
            && !chars[start_pos].is_whitespace()
            && chars[start_pos] != '.'
            && chars[start_pos] != ','
        {
            start_pos = start_pos.saturating_sub(1);
        }

        // If we stopped on whitespace or punctuation, move forward one
        if start_pos > 0
            && (chars[start_pos].is_whitespace()
                || chars[start_pos] == '.'
                || chars[start_pos] == ',')
        {
            start_pos += 1;
        }

        // Remove the text
        let before: String = chars[..start_pos].iter().collect();
        let after: String = chars[end_pos..].iter().collect();
        self.lines[self.cursor_line] = format!("{}{}", before, after);
        self.cursor_col = start_pos;
    }
}

/// Notification system for TUI messages
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationType {
    Success,
    Error,
    Info,
    Warning,
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub message: String,
    pub notification_type: NotificationType,
    pub created_at: Instant,
    pub duration: Duration,
}

impl Notification {
    pub fn success(message: String) -> Self {
        Self {
            message,
            notification_type: NotificationType::Success,
            created_at: Instant::now(),
            duration: Duration::from_secs(3),
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            message,
            notification_type: NotificationType::Error,
            created_at: Instant::now(),
            duration: Duration::from_secs(5),
        }
    }

    pub fn info(message: String) -> Self {
        Self {
            message,
            notification_type: NotificationType::Info,
            created_at: Instant::now(),
            duration: Duration::from_secs(3),
        }
    }

    pub fn warning(message: String) -> Self {
        Self {
            message,
            notification_type: NotificationType::Warning,
            created_at: Instant::now(),
            duration: Duration::from_secs(4),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.duration
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusedPane {
    Sessions, // Left pane - workspace/session list
    LiveLogs, // Right pane - live logs
}

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
    ClaudeChat, // Claude chat popup overlay
    GitView,    // Git status and diff view
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

#[derive(Debug, Clone)]
pub struct ClaudeChatState {
    pub messages: Vec<ClaudeMessage>,
    pub input_buffer: String,
    pub is_streaming: bool,
    pub current_streaming_response: Option<String>,
    pub associated_session_id: Option<Uuid>,
    pub total_tokens_used: u32,
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

impl ClaudeChatState {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            input_buffer: String::new(),
            is_streaming: false,
            current_streaming_response: None,
            associated_session_id: None,
            total_tokens_used: 0,
            last_activity: chrono::Utc::now(),
        }
    }

    pub fn add_message(&mut self, message: ClaudeMessage) {
        self.messages.push(message);
        self.last_activity = chrono::Utc::now();
    }

    pub fn start_streaming(&mut self, user_message: String) {
        self.add_message(ClaudeMessage::user(user_message));
        self.is_streaming = true;
        self.current_streaming_response = Some(String::new());
        self.input_buffer.clear();
        self.last_activity = chrono::Utc::now();
    }

    pub fn append_streaming_response(&mut self, text: &str) {
        if let Some(ref mut response) = self.current_streaming_response {
            response.push_str(text);
        }
        self.last_activity = chrono::Utc::now();
    }

    pub fn finish_streaming(&mut self) {
        if let Some(response) = self.current_streaming_response.take() {
            self.add_message(ClaudeMessage::assistant(response));
        }
        self.is_streaming = false;
    }

    pub fn clear_input(&mut self) {
        self.input_buffer.clear();
    }

    pub fn add_char_to_input(&mut self, ch: char) {
        if !self.is_streaming {
            self.input_buffer.push(ch);
        }
    }

    pub fn backspace_input(&mut self) {
        if !self.is_streaming {
            self.input_buffer.pop();
        }
    }
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

    // Claude chat visibility toggle
    pub claude_chat_visible: bool,

    // Focus management for panes
    pub focused_pane: FocusedPane,
    // Track if current directory is a git repository
    pub is_current_dir_git_repo: bool,
    // Track which session logs were last fetched to avoid unnecessary refetches
    pub last_logs_session_id: Option<Uuid>,
    // Track attached terminal state
    pub attached_session_id: Option<Uuid>,
    // Auth setup state
    pub auth_setup_state: Option<AuthSetupState>,
    // Track when logs were last updated for each session
    pub log_last_updated: HashMap<Uuid, std::time::Instant>,
    // Track the last time we checked for log updates globally
    pub last_log_check: Option<std::time::Instant>,
    // Track the last time we checked for OAuth token refresh
    pub last_token_refresh_check: Option<std::time::Instant>,
    // Claude chat integration
    pub claude_chat_state: Option<ClaudeChatState>,
    // Live logs from Docker containers
    pub live_logs: HashMap<Uuid, Vec<LogEntry>>,
    // Claude API client manager (when initialized)
    pub claude_manager: Option<ClaudeChatManager>,
    // Docker log streaming coordinator
    pub log_streaming_coordinator: Option<LogStreamingCoordinator>,
    // Channel sender for log streaming
    pub log_sender: Option<mpsc::UnboundedSender<(Uuid, LogEntry)>>,
    // Git view state
    pub git_view_state: Option<crate::components::GitViewState>,
    // Notification system
    pub notifications: Vec<Notification>,
    // Pending event to be processed in next loop iteration
    pub pending_event: Option<crate::app::events::AppEvent>,

    // Quick commit dialog state
    pub quick_commit_message: Option<String>, // None = not in quick commit mode, Some = message being entered
    pub quick_commit_cursor: usize,           // Cursor position in quick commit message
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
    pub skip_permissions: bool,    // true to use --dangerously-skip-permissions flag
    pub mode: crate::models::SessionMode, // Interactive or Boss mode
    pub boss_prompt: TextEditor,   // The prompt text editor for boss mode execution
    pub file_finder: FuzzyFileFinderState, // Fuzzy file finder for @ symbol
    pub restart_session_id: Option<Uuid>, // If set, this is a restart operation
}

impl Default for NewSessionState {
    fn default() -> Self {
        Self {
            available_repos: vec![],
            filtered_repos: vec![],
            selected_repo_index: None,
            branch_name: String::new(),
            step: NewSessionStep::SelectRepo,
            filter_text: String::new(),
            is_current_dir_mode: false,
            skip_permissions: false,
            mode: crate::models::SessionMode::Interactive,
            boss_prompt: TextEditor::new(),
            file_finder: FuzzyFileFinderState::new(),
            restart_session_id: None,
        }
    }
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
    SelectMode,  // Choose between Interactive and Boss mode
    InputPrompt, // Enter prompt for Boss mode
    ConfigurePermissions,
    Creating,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AsyncAction {
    StartNewSession,        // Old - will be removed
    StartWorkspaceSearch,   // New - search all workspaces
    NewSessionInCurrentDir, // New - create session in current directory
    NewSessionNormal,       // New - create normal new session with mode selection
    CreateNewSession,
    DeleteSession(Uuid),       // New - delete session with container cleanup
    RefreshWorkspaces,         // Manual refresh of workspace data
    FetchContainerLogs(Uuid),  // Fetch container logs for a session
    AttachToContainer(Uuid),   // Attach to a container session
    KillContainer(Uuid),       // Kill container for a session
    AuthSetupOAuth,            // Run OAuth authentication setup
    AuthSetupApiKey,           // Save API key authentication
    ReauthenticateCredentials, // Re-authenticate Claude credentials
    RestartSession(Uuid),      // Restart a stopped session with new container
    CleanupOrphaned,           // Clean up orphaned containers without worktrees
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
            claude_chat_visible: false,
            focused_pane: FocusedPane::Sessions,
            is_current_dir_git_repo: false,
            last_logs_session_id: None,
            attached_session_id: None,
            auth_setup_state: None,
            log_last_updated: HashMap::new(),
            last_log_check: None,
            last_token_refresh_check: None,
            claude_chat_state: None,
            live_logs: HashMap::new(),
            claude_manager: None,
            log_streaming_coordinator: None,
            log_sender: None,
            git_view_state: None,
            notifications: Vec::new(),
            pending_event: None,

            // Initialize quick commit state
            quick_commit_message: None,
            quick_commit_cursor: 0,
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize Claude integration if authentication is available
    pub async fn init_claude_integration(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match ClaudeApiClient::load_auth_from_config() {
            Ok(auth) => {
                info!("Initializing Claude API integration");
                match ClaudeApiClient::with_auth(auth) {
                    Ok(client) => {
                        // Test connection
                        match client.test_connection().await {
                            Ok(()) => {
                                let mut manager = ClaudeChatManager::new(client);
                                manager.create_session(None);
                                self.claude_manager = Some(manager);
                                self.claude_chat_state = Some(ClaudeChatState::new());
                                info!("Claude integration initialized successfully");
                                Ok(())
                            }
                            Err(e) => {
                                warn!("Claude API connection test failed: {}", e);
                                Err(format!("Claude API connection failed: {}", e).into())
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to create Claude API client: {}", e);
                        Err(e.into())
                    }
                }
            }
            Err(e) => {
                info!("Claude authentication not configured: {}", e);
                // This is OK - user can set up auth later
                Ok(())
            }
        }
    }

    /// Send a message to Claude
    pub async fn send_claude_message(
        &mut self,
        message: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let (Some(chat_state), Some(manager)) =
            (&mut self.claude_chat_state, &mut self.claude_manager)
        {
            chat_state.start_streaming(message.clone());

            // Start streaming response
            match manager.stream_message(&message).await {
                Ok(mut stream) => {
                    // Handle streaming response
                    while let Some(event) = stream.next().await {
                        match event {
                            Ok(ClaudeStreamingEvent::ContentBlockDelta { delta, .. }) => {
                                chat_state.append_streaming_response(&delta.text);
                                self.ui_needs_refresh = true;
                            }
                            Ok(ClaudeStreamingEvent::MessageStop) => {
                                chat_state.finish_streaming();
                                self.ui_needs_refresh = true;
                                break;
                            }
                            Ok(ClaudeStreamingEvent::Error { error }) => {
                                error!("Claude API error: {}", error.message);
                                chat_state.finish_streaming();
                                return Err(format!("Claude error: {}", error.message).into());
                            }
                            Ok(_) => {
                                // Other events - continue
                            }
                            Err(e) => {
                                error!("Streaming error: {}", e);
                                chat_state.finish_streaming();
                                return Err(e.into());
                            }
                        }
                    }
                    Ok(())
                }
                Err(e) => {
                    chat_state.is_streaming = false;
                    Err(e.into())
                }
            }
        } else {
            Err("Claude integration not initialized".into())
        }
    }

    /// Add a log entry to live logs
    pub fn add_live_log(&mut self, session_id: Uuid, log_entry: LogEntry) {
        self.live_logs.entry(session_id).or_insert_with(Vec::new).push(log_entry);

        // Limit log entries to prevent memory issues (keep last 1000)
        if let Some(logs) = self.live_logs.get_mut(&session_id) {
            if logs.len() > 1000 {
                logs.drain(0..logs.len() - 1000);
            }
        }

        self.ui_needs_refresh = true;
    }

    /// Start log streaming for a session when it becomes active
    pub async fn start_log_streaming_for_session(
        &mut self,
        session_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(coordinator) = &mut self.log_streaming_coordinator {
            // Find the session to get container info
            let session_info = self
                .workspaces
                .iter()
                .flat_map(|w| &w.sessions)
                .find(|s| s.id == session_id)
                .and_then(|s| {
                    s.container_id.clone().map(|container_id| {
                        (
                            container_id,
                            format!("{}-{}", s.name, s.branch_name),
                            s.mode.clone(),
                        )
                    })
                });

            if let Some((container_id, container_name, session_mode)) = session_info {
                info!(
                    "Starting log streaming for session {} (container: {})",
                    session_id, container_id
                );
                coordinator
                    .start_streaming(session_id, container_id, container_name, session_mode)
                    .await?;
            }
        }
        Ok(())
    }

    /// Stop log streaming for a session when it becomes inactive
    pub async fn stop_log_streaming_for_session(
        &mut self,
        session_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(coordinator) = &mut self.log_streaming_coordinator {
            info!("Stopping log streaming for session {}", session_id);
            coordinator.stop_streaming(session_id).await?;
        }
        Ok(())
    }

    /// Clear live logs for a session
    pub fn clear_live_logs(&mut self, session_id: Uuid) {
        self.live_logs.remove(&session_id);
        self.ui_needs_refresh = true;
    }

    /// Get total live log count across all sessions
    pub fn total_live_log_count(&self) -> usize {
        self.live_logs.values().map(|logs| logs.len()).sum()
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
        // If we have a refresh token, we can refresh expired access tokens, so it's not "first time setup"
        let has_valid_oauth = if has_credentials && has_claude_json {
            // Check if we have OAuth credentials (either valid token OR refresh token to get new one)
            let credentials_path = auth_dir.join(".credentials.json");
            if let Ok(contents) = std::fs::read_to_string(&credentials_path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&contents) {
                    if let Some(oauth) = json.get("claudeAiOauth") {
                        // If we have a refresh token, we can refresh even if access token is expired
                        if oauth.get("refreshToken").is_some() {
                            info!("Found refresh token - can refresh if needed");
                            true
                        } else {
                            // No refresh token, check if access token is still valid
                            Self::is_oauth_token_valid(&credentials_path)
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        // Show auth screen if we don't have valid OAuth setup AND no API key alternatives
        !has_valid_oauth && !has_api_key && !has_env_api_key
    }

    /// Check if OAuth token in credentials file is still valid (not expired)
    fn is_oauth_token_valid(credentials_path: &std::path::Path) -> bool {
        use std::fs;

        if let Ok(contents) = fs::read_to_string(credentials_path) {
            // Parse the JSON to extract OAuth token info
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&contents) {
                if let Some(oauth) = json.get("claudeAiOauth") {
                    if let Some(expires_at) = oauth.get("expiresAt").and_then(|v| v.as_u64()) {
                        // Check if current time is before expiration time
                        let current_time = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;

                        if current_time < expires_at {
                            info!(
                                "OAuth token is valid, expires at: {}",
                                chrono::DateTime::from_timestamp_millis(expires_at as i64)
                                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                                    .unwrap_or_else(|| "unknown".to_string())
                            );
                            return true;
                        } else {
                            warn!(
                                "OAuth token has expired at: {}",
                                chrono::DateTime::from_timestamp_millis(expires_at as i64)
                                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                                    .unwrap_or_else(|| "unknown".to_string())
                            );
                            return false;
                        }
                    }
                }
            }
        }

        // If we can't parse or find expiration info, assume invalid
        warn!("Could not validate OAuth token from credentials file");
        false
    }

    /// Check if OAuth token needs refresh (expires within 30 minutes)
    fn oauth_token_needs_refresh(credentials_path: &std::path::Path) -> bool {
        use std::fs;

        if let Ok(contents) = fs::read_to_string(credentials_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&contents) {
                if let Some(oauth) = json.get("claudeAiOauth") {
                    // Check if we have a refresh token
                    if oauth.get("refreshToken").is_none() {
                        info!("No refresh token available");
                        return false;
                    }

                    if let Some(expires_at) = oauth.get("expiresAt").and_then(|v| v.as_u64()) {
                        let current_time = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;

                        // Refresh if token expires in less than 30 minutes
                        let buffer_time = 30 * 60 * 1000; // 30 minutes in milliseconds

                        if current_time >= (expires_at.saturating_sub(buffer_time)) {
                            info!(
                                "OAuth token needs refresh, expires at: {}",
                                chrono::DateTime::from_timestamp_millis(expires_at as i64)
                                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                                    .unwrap_or_else(|| "unknown".to_string())
                            );
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// Refresh OAuth tokens using the refresh token
    pub async fn refresh_oauth_tokens(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Attempting to refresh OAuth tokens");

        let home_dir = dirs::home_dir().ok_or("Could not determine home directory")?;
        let auth_dir = home_dir.join(".claude-in-a-box").join("auth");
        let credentials_path = auth_dir.join(".credentials.json");

        // Check if tokens actually need refresh
        if !Self::oauth_token_needs_refresh(&credentials_path) {
            info!("OAuth tokens do not need refresh yet");
            return Ok(());
        }

        // Build the Docker image if needed
        let image_name = "claude-box:claude-dev";
        let image_check = tokio::process::Command::new("docker")
            .args(["image", "inspect", image_name])
            .output()
            .await?;

        if !image_check.status.success() {
            info!("Building claude-dev image for token refresh...");
            let build_status = tokio::process::Command::new("docker")
                .args(["build", "-t", image_name, "docker/claude-dev"])
                .status()
                .await?;

            if !build_status.success() {
                return Err("Failed to build image for token refresh".into());
            }
        }

        // Run the oauth-refresh.js script in a container (with retries built-in)
        info!("Running OAuth token refresh in container");

        // Create the volume mount string that will live long enough
        let volume_mount = format!("{}:/home/claude-user/.claude", auth_dir.display());

        // Build args based on debug mode
        let mut args = vec![
            "run",
            "--rm",
            "-v",
            &volume_mount,
            "-e",
            "PATH=/home/claude-user/.npm-global/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            "-e",
            "HOME=/home/claude-user",
        ];

        // Add debug env if needed
        // Check if we're in debug mode by checking RUST_LOG env var
        if std::env::var("RUST_LOG").unwrap_or_default().contains("debug") {
            args.push("-e");
            args.push("DEBUG=1");
        }

        args.extend([
            "-w",
            "/home/claude-user",
            "--user",
            "claude-user",
            "--entrypoint",
            "node",
            image_name,
            "/app/scripts/oauth-refresh.js",
        ]);

        let output = tokio::process::Command::new("docker")
            .args(&args)
            .output()
            .await?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            info!("OAuth token refresh successful: {}", stdout.trim());

            // Verify the new token is valid
            if Self::is_oauth_token_valid(&credentials_path) {
                info!("New OAuth token verified as valid");
                Ok(())
            } else {
                Err("Token refresh succeeded but new token is invalid".into())
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            warn!("OAuth token refresh failed");
            warn!("Stderr: {}", stderr.trim());
            warn!("Stdout: {}", stdout.trim());
            Err(format!("Token refresh failed: {}", stderr.trim()).into())
        }
    }

    pub fn check_current_directory_status(&mut self) {
        use crate::git::workspace_scanner::WorkspaceScanner;
        use std::env;

        if let Ok(current_dir) = env::current_dir() {
            self.is_current_dir_git_repo =
                WorkspaceScanner::validate_workspace(&current_dir).unwrap_or(false);

            if !self.is_current_dir_git_repo {
                info!(
                    "Current directory is not a git repository: {:?}",
                    current_dir
                );
                self.current_view = View::NonGitNotification;
            } else {
                info!(
                    "Current directory is a valid git repository: {:?}",
                    current_dir
                );
            }
        } else {
            warn!("Could not determine current directory");
            self.is_current_dir_git_repo = false;
            self.current_view = View::NonGitNotification;
        }
    }

    pub async fn load_real_workspaces(&mut self) {
        info!("Loading active sessions from Docker containers");

        // Check and refresh OAuth tokens if needed
        let home_dir = dirs::home_dir();
        if let Some(home) = home_dir {
            let credentials_path = home.join(".claude-in-a-box").join("auth").join(".credentials.json");

            // Only attempt refresh if we have OAuth credentials
            if credentials_path.exists() && Self::oauth_token_needs_refresh(&credentials_path) {
                info!("OAuth token needs refresh, attempting automatic refresh");
                match self.refresh_oauth_tokens().await {
                    Ok(()) => info!("OAuth tokens refreshed successfully"),
                    Err(e) => warn!("Failed to refresh OAuth tokens: {}", e),
                }
            }
        }

        // Try to load active sessions
        match SessionLoader::new().await {
            Ok(loader) => {
                match loader.load_active_sessions().await {
                    Ok(workspaces) => {
                        self.workspaces = workspaces;
                        info!(
                            "Loaded {} workspaces with active sessions",
                            self.workspaces.len()
                        );

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

        let mut session1 = Session::new(
            "fix-auth".to_string(),
            workspace1.path.to_string_lossy().to_string(),
        );
        session1.set_status(crate::models::SessionStatus::Running);
        session1.git_changes.added = 42;
        session1.git_changes.deleted = 13;

        let mut session2 = Session::new(
            "add-feature".to_string(),
            workspace1.path.to_string_lossy().to_string(),
        );
        session2.set_status(crate::models::SessionStatus::Stopped);

        let mut session3 = Session::new(
            "debug-issue".to_string(),
            workspace1.path.to_string_lossy().to_string(),
        );
        session3.set_status(crate::models::SessionStatus::Error(
            "Container failed to start".to_string(),
        ));

        workspace1.add_session(session1);
        workspace1.add_session(session2);
        workspace1.add_session(session3);

        let mut workspace2 = Workspace::new(
            "project2".to_string(),
            "/Users/user/projects/project2".into(),
        );

        let mut session4 = Session::new(
            "refactor-api".to_string(),
            workspace2.path.to_string_lossy().to_string(),
        );
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

        info!(
            "Loaded large mock dataset with {} workspaces",
            self.workspaces.len()
        );
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
                    self.selected_session_index = Some(if current == 0 {
                        workspace.sessions.len() - 1
                    } else {
                        current - 1
                    });
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
            self.selected_session_index =
                if !self.workspaces[self.selected_workspace_index.unwrap()].sessions.is_empty() {
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
            self.selected_workspace_index = Some(if current == 0 {
                self.workspaces.len() - 1
            } else {
                current - 1
            });
            self.selected_session_index =
                if !self.workspaces[self.selected_workspace_index.unwrap()].sessions.is_empty() {
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

    pub fn toggle_claude_chat(&mut self) {
        if self.current_view == View::ClaudeChat {
            // Close Claude chat popup and return to main view
            self.current_view = View::SessionList;
            self.claude_chat_visible = false;
        } else {
            // Open Claude chat popup
            self.current_view = View::ClaudeChat;
            self.claude_chat_visible = true;
        }
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

    /// Get a reference to the currently selected session
    pub fn get_selected_session(&self) -> Option<&crate::models::Session> {
        let workspace_idx = self.selected_workspace_index?;
        let session_idx = self.selected_session_index?;

        self.workspaces.get(workspace_idx)?.sessions.get(session_idx)
    }

    /// Attach to a container session using docker exec with proper terminal handling
    pub async fn attach_to_container(
        &mut self,
        session_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::docker::ContainerManager;

        // Find the session to get container ID
        let container_id = self
            .workspaces
            .iter()
            .flat_map(|w| &w.sessions)
            .find(|s| s.id == session_id)
            .and_then(|s| s.container_id.as_ref())
            .cloned();

        if let Some(container_id) = container_id {
            info!(
                "Attaching to container {} for session {}",
                container_id, session_id
            );

            // Check if container is running
            let container_manager = ContainerManager::new().await?;
            let status = container_manager.get_container_status(&container_id).await?;

            match status {
                crate::docker::ContainerStatus::Running => {
                    // Start an interactive bash shell instead of Claude CLI directly
                    // This gives users more flexibility to run claude when needed
                    // Force bash to read .bashrc to load custom session environment
                    let exec_command = vec![
                        "/bin/bash".to_string(),
                        "-l".to_string(), // Login shell to read .bash_profile/.bashrc
                        "-i".to_string(), // Interactive shell
                    ];

                    match container_manager
                        .exec_interactive_blocking(&container_id, exec_command)
                        .await
                    {
                        Ok(_exit_status) => {
                            info!(
                                "Successfully detached from container {} for session {}",
                                container_id, session_id
                            );
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
                    warn!(
                        "Cannot attach to container {} - it is not running (status: {:?})",
                        container_id, status
                    );
                    Err(format!("Container is not running (status: {:?})", status).into())
                }
            }
        } else {
            warn!(
                "Cannot attach to session {} - no container ID found",
                session_id
            );
            Err("No container associated with this session".into())
        }
    }

    /// Kill the container for a session (force stop and cleanup)
    pub async fn kill_container(
        &mut self,
        session_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::docker::ContainerManager;

        // Find the session to get container ID
        let container_id = self
            .workspaces
            .iter()
            .flat_map(|w| &w.sessions)
            .find(|s| s.id == session_id)
            .and_then(|s| s.container_id.as_ref())
            .cloned();

        if let Some(container_id) = container_id {
            info!(
                "Killing container {} for session {}",
                container_id, session_id
            );

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

                info!(
                    "Successfully killed and removed container {} for session {}",
                    container_id, session_id
                );
            }

            Ok(())
        } else {
            warn!(
                "Cannot kill container for session {} - no container ID found",
                session_id
            );
            Err("No container associated with this session".into())
        }
    }

    /// Helper method to find a session container by session ID
    fn find_session_container_mut(
        &mut self,
        _session_id: Uuid,
    ) -> Option<&mut crate::docker::SessionContainer> {
        // This is a simplified approach - in a real implementation you'd need to track
        // SessionContainer objects separately or modify the Session model to include them
        None // Placeholder - would need container tracking
    }

    /// Fetch container logs for a session
    pub async fn fetch_container_logs(
        &mut self,
        session_id: Uuid,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        use crate::docker::ContainerManager;

        // Find the session to get container ID
        let container_id = self
            .workspaces
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
            Ok(self
                .logs
                .get(&session_id)
                .cloned()
                .unwrap_or_else(|| vec!["No container associated with this session".to_string()]))
        }
    }

    /// Fetch Claude-specific logs from the container
    pub async fn fetch_claude_logs(
        &mut self,
        session_id: Uuid,
    ) -> Result<String, Box<dyn std::error::Error>> {
        use crate::docker::ContainerManager;

        // Find the session to get container ID and update recent_logs
        let container_id = self
            .workspaces
            .iter_mut()
            .flat_map(|w| &mut w.sessions)
            .find(|s| s.id == session_id)
            .and_then(|s| {
                let id = s.container_id.clone();
                // We'll update recent_logs after fetching
                id
            });

        if let Some(container_id) = container_id {
            let container_manager = ContainerManager::new().await?;
            let logs = container_manager.tail_logs(&container_id, 20).await?;

            // Update the session's recent_logs field
            if let Some(session) = self
                .workspaces
                .iter_mut()
                .flat_map(|w| &mut w.sessions)
                .find(|s| s.id == session_id)
            {
                session.recent_logs = Some(logs.clone());
            }

            Ok(logs)
        } else {
            Ok("No container associated with this session".to_string())
        }
    }

    pub async fn new_session_normal(&mut self) {
        use crate::git::WorkspaceScanner;
        use std::env;

        info!("Starting normal new session with mode selection");

        // Check if authentication is set up first
        if Self::is_first_time_setup() {
            info!("Authentication not set up, switching to auth setup view");
            self.current_view = View::AuthSetup;
            self.auth_setup_state = Some(AuthSetupState {
                selected_method: AuthMethod::OAuth,
                api_key_input: String::new(),
                is_processing: false,
                error_message: Some("Authentication required before creating sessions.\n\nPlease set up Claude authentication to continue.".to_string()),
                show_cursor: false,
            });
            return;
        }

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
                info!(
                    "Current directory is a valid git repository: {:?}",
                    current_dir
                );
            }
            Ok(false) => {
                warn!(
                    "Current directory is not a git repository: {:?}",
                    current_dir
                );
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
        let branch_base = format!(
            "claude/{}",
            uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("session")
        );

        // Create new session state for normal new session (NOT current directory mode)
        self.new_session_state = Some(NewSessionState {
            available_repos: vec![current_dir.clone()],
            filtered_repos: vec![(0, current_dir.clone())],
            selected_repo_index: Some(0),
            branch_name: branch_base.clone(),
            step: NewSessionStep::InputBranch,
            ..Default::default()
        });

        self.current_view = View::NewSession;

        info!(
            "Successfully created normal new session state with branch: {}",
            branch_base
        );
    }

    pub async fn new_session_in_current_dir(&mut self) {
        use crate::git::WorkspaceScanner;
        use std::env;

        info!("Starting new session in current directory");

        // Check if authentication is set up first
        if Self::is_first_time_setup() {
            info!("Authentication not set up, switching to auth setup view");
            self.current_view = View::AuthSetup;
            self.auth_setup_state = Some(AuthSetupState {
                selected_method: AuthMethod::OAuth,
                api_key_input: String::new(),
                is_processing: false,
                error_message: Some("Authentication required before creating sessions.\n\nPlease set up Claude authentication to continue.".to_string()),
                show_cursor: false,
            });
            return;
        }

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
                info!(
                    "Current directory is a valid git repository: {:?}",
                    current_dir
                );
            }
            Ok(false) => {
                warn!(
                    "Current directory is not a git repository: {:?}",
                    current_dir
                );
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
        let branch_base = format!(
            "claude/{}",
            uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("session")
        );

        // Create new session state for current directory
        self.new_session_state = Some(NewSessionState {
            available_repos: vec![current_dir.clone()],
            filtered_repos: vec![(0, current_dir.clone())],
            selected_repo_index: Some(0),
            branch_name: branch_base.clone(),
            step: NewSessionStep::InputBranch,
            is_current_dir_mode: true,
            ..Default::default()
        });

        self.current_view = View::NewSession;

        info!(
            "Successfully created new session state with branch: {}",
            branch_base
        );
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
                        let branch_base = format!(
                            "claude/{}",
                            uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("session")
                        );

                        // Initialize filtered repos with all repos (even if empty)
                        let filtered_repos: Vec<(usize, std::path::PathBuf)> = repos
                            .iter()
                            .enumerate()
                            .map(|(idx, path)| (idx, path.clone()))
                            .collect();

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
                            ..Default::default()
                        });

                        self.current_view = View::SearchWorkspace;
                        info!("Successfully transitioned to SearchWorkspace view");
                    }
                    Err(e) => {
                        warn!("Failed to load repositories: {}", e);
                        // Still transition to search view with empty state
                        self.new_session_state = Some(NewSessionState {
                            branch_name: format!(
                                "claude/{}",
                                uuid::Uuid::new_v4()
                                    .to_string()
                                    .split('-')
                                    .next()
                                    .unwrap_or("session")
                            ),
                            ..Default::default()
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
                    branch_name: format!(
                        "claude/{}",
                        uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("session")
                    ),
                    ..Default::default()
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
                        let filtered_repos: Vec<(usize, std::path::PathBuf)> = repos
                            .iter()
                            .enumerate()
                            .map(|(idx, path)| (idx, path.clone()))
                            .collect();

                        self.new_session_state = Some(NewSessionState {
                            available_repos: repos,
                            filtered_repos,
                            selected_repo_index: if has_repos { Some(0) } else { None },
                            ..Default::default()
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
                state.selected_repo_index = Some(if current == 0 {
                    state.filtered_repos.len() - 1
                } else {
                    current - 1
                });
            }
        }
    }

    pub fn new_session_confirm_repo(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.selected_repo_index.is_some() {
                tracing::info!(
                    "Confirming repository selection - selected_repo_index: {:?}",
                    state.selected_repo_index
                );
                tracing::info!(
                    "Available repos count: {}, Filtered repos count: {}",
                    state.available_repos.len(),
                    state.filtered_repos.len()
                );

                if let Some(repo_index) = state.selected_repo_index {
                    if let Some((_, repo_path)) = state.filtered_repos.get(repo_index) {
                        tracing::info!("Selected repository path: {:?}", repo_path);
                    } else {
                        tracing::error!(
                            "Failed to get repository at index {} from filtered_repos",
                            repo_index
                        );
                        return;
                    }
                }

                state.step = NewSessionStep::InputBranch;
                let uuid_str = uuid::Uuid::new_v4().to_string();
                state.branch_name = format!("claude-session-{}", &uuid_str[..8]);

                // Change view from SearchWorkspace to NewSession to show branch input
                self.current_view = View::NewSession;
                tracing::info!(
                    "Repository confirmed, transitioning to branch input step with branch: {}",
                    state.branch_name
                );
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

    pub fn new_session_proceed_to_mode_selection(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputBranch {
                tracing::info!(
                    "Proceeding from InputBranch to SelectMode with branch: {}",
                    state.branch_name
                );
                state.step = NewSessionStep::SelectMode;
            }
        }
    }

    pub fn new_session_proceed_from_mode(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::SelectMode {
                tracing::info!(
                    "Proceeding from SelectMode to next step with mode: {:?}",
                    state.mode
                );
                match state.mode {
                    crate::models::SessionMode::Interactive => {
                        // Interactive mode: go directly to permissions
                        state.step = NewSessionStep::ConfigurePermissions;
                        tracing::info!("Interactive mode selected, going to ConfigurePermissions");
                    }
                    crate::models::SessionMode::Boss => {
                        // Boss mode: go to prompt input first
                        state.step = NewSessionStep::InputPrompt;
                        tracing::info!("Boss mode selected, going to InputPrompt");
                    }
                }
            }
        }
    }

    pub fn new_session_proceed_to_permissions(&mut self) {
        tracing::info!("new_session_proceed_to_permissions called");
        if let Some(ref mut state) = self.new_session_state {
            tracing::debug!("Current session state step: {:?}", state.step);
            if state.step == NewSessionStep::InputPrompt {
                tracing::info!("Advancing from InputPrompt to ConfigurePermissions");
                state.step = NewSessionStep::ConfigurePermissions;
                self.ui_needs_refresh = true;
            } else {
                tracing::warn!(
                    "Cannot proceed to permissions - not in InputPrompt step (current: {:?})",
                    state.step
                );
            }
        } else {
            tracing::error!("Cannot proceed to permissions - no session state found");
        }
    }

    pub fn new_session_toggle_mode(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::SelectMode {
                state.mode = match state.mode {
                    crate::models::SessionMode::Interactive => crate::models::SessionMode::Boss,
                    crate::models::SessionMode::Boss => crate::models::SessionMode::Interactive,
                };
            }
        }
    }

    pub fn new_session_add_char_to_prompt(&mut self, ch: char) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt {
                if ch == '@' {
                    // Activate fuzzy file finder (supports multiple @ references)
                    let workspace_root = if let Some(selected_idx) = state.selected_repo_index {
                        state.filtered_repos.get(selected_idx).map(|(_, path)| path.clone())
                    } else {
                        None
                    };
                    // If already active, deactivate current search and start new one
                    if state.file_finder.is_active {
                        state.file_finder.deactivate();
                    }
                    state.file_finder.activate(state.boss_prompt.to_string().len(), workspace_root);
                    state.boss_prompt.insert_char(ch);
                } else if state.file_finder.is_active {
                    // File finder is active, handle character input for filtering
                    if ch == ' ' || ch == '\t' || ch == '\n' {
                        // Whitespace deactivates file finder
                        state.file_finder.deactivate();
                        state.boss_prompt.insert_char(ch);
                    } else {
                        state.file_finder.add_char_to_query(ch);
                    }
                } else {
                    // Normal character input
                    state.boss_prompt.insert_char(ch);
                }
            }
        }
    }

    pub fn new_session_backspace_prompt(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt {
                if state.file_finder.is_active {
                    if !state.file_finder.query.is_empty() {
                        // Remove character from file finder query
                        state.file_finder.backspace_query();
                    } else {
                        // Query is empty, deactivate file finder and remove @ symbol
                        state.file_finder.deactivate();
                        state.boss_prompt.backspace();
                    }
                } else {
                    // Normal backspace
                    state.boss_prompt.backspace();
                }
            }
        }
    }

    pub fn new_session_move_cursor_left(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.move_cursor_left();
            }
        }
    }

    pub fn new_session_move_cursor_right(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.move_cursor_right();
            }
        }
    }

    pub fn new_session_move_cursor_up(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.move_cursor_up();
            }
        }
    }

    pub fn new_session_move_cursor_down(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.move_cursor_down();
            }
        }
    }

    pub fn new_session_move_to_line_start(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.move_to_line_start();
            }
        }
    }

    pub fn new_session_move_to_line_end(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.move_to_line_end();
            }
        }
    }

    pub fn new_session_move_cursor_word_left(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.move_cursor_word_backward();
            }
        }
    }

    pub fn new_session_move_cursor_word_right(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.move_cursor_word_forward();
            }
        }
    }

    pub fn new_session_delete_word_forward(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.delete_word_forward();
            }
        }
    }

    pub fn new_session_delete_word_backward(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.delete_word_backward();
            }
        }
    }

    pub fn new_session_insert_newline(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.insert_newline();
            }
        }
    }

    pub fn new_session_paste_text(&mut self, text: String) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                // Insert the pasted text at the current cursor position
                state.boss_prompt.insert_text(&text);
            }
        }
    }

    pub fn new_session_toggle_permissions(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::ConfigurePermissions {
                state.skip_permissions = !state.skip_permissions;
            }
        }
    }

    pub async fn new_session_create(&mut self) {
        // Check if authentication is set up first
        if Self::is_first_time_setup() {
            info!("Authentication not set up, switching to auth setup view");
            self.current_view = View::AuthSetup;
            self.auth_setup_state = Some(AuthSetupState {
                selected_method: AuthMethod::OAuth,
                api_key_input: String::new(),
                is_processing: false,
                error_message: Some("Authentication required before creating sessions.\n\nPlease set up Claude authentication to continue.".to_string()),
                show_cursor: false,
            });
            // Clear new session state
            self.new_session_state = None;
            return;
        }

        let (
            repo_path,
            branch_name,
            session_id,
            skip_permissions,
            mode,
            boss_prompt,
            restart_session_id,
        ) = {
            if let Some(ref mut state) = self.new_session_state {
                tracing::info!("new_session_create called with step: {:?}", state.step);

                // Handle both ConfigurePermissions step (normal flow) and InputBranch step (current dir mode)
                let can_create = match state.step {
                    NewSessionStep::ConfigurePermissions => true,
                    NewSessionStep::InputBranch if state.is_current_dir_mode => {
                        // For current directory mode, skip to permissions step with defaults
                        state.step = NewSessionStep::ConfigurePermissions;
                        state.skip_permissions = false; // Default to safe permissions
                        state.mode = crate::models::SessionMode::Interactive; // Default mode
                        true
                    }
                    _ => false,
                };

                if can_create {
                    if let Some(repo_index) = state.selected_repo_index {
                        if let Some((_, repo_path)) = state.filtered_repos.get(repo_index) {
                            tracing::info!(
                                "Creating session for repository: {:?}, branch: {}",
                                repo_path,
                                state.branch_name
                            );
                            state.step = NewSessionStep::Creating;

                            // Use existing session ID for restart, or generate new one
                            let session_id =
                                state.restart_session_id.unwrap_or_else(|| uuid::Uuid::new_v4());

                            (
                                repo_path.clone(),
                                state.branch_name.clone(),
                                session_id,
                                state.skip_permissions,
                                state.mode.clone(),
                                if state.mode == crate::models::SessionMode::Boss {
                                    Some(state.boss_prompt.to_string())
                                } else {
                                    None
                                },
                                state.restart_session_id, // Pass restart session ID
                            )
                        } else {
                            tracing::error!(
                                "Failed to get repository path from filtered_repos at index: {}",
                                repo_index
                            );
                            return;
                        }
                    } else {
                        tracing::error!("No repository selected (selected_repo_index is None)");
                        return;
                    }
                } else {
                    tracing::warn!(
                        "new_session_create called but step is not valid for creation, current step: {:?}, is_current_dir_mode: {}",
                        state.step,
                        state.is_current_dir_mode
                    );
                    return;
                }
            } else {
                tracing::error!("new_session_create called but new_session_state is None");
                return;
            }
        };

        // Create the session with log streaming
        tracing::info!(
            "Calling create_session_with_logs for session {} (mode: {:?}, restart: {})",
            session_id,
            mode,
            restart_session_id.is_some()
        );

        let result = if let Some(restart_id) = restart_session_id {
            // This is a restart - try to reuse existing worktree
            info!(
                "Restarting session {} with potentially updated configuration",
                restart_id
            );
            self.create_restart_session_with_logs(
                &repo_path,
                &branch_name,
                session_id,
                skip_permissions,
                mode,
                boss_prompt,
            )
            .await
        } else {
            // Normal new session creation
            self.create_session_with_logs(
                &repo_path,
                &branch_name,
                session_id,
                skip_permissions,
                mode,
                boss_prompt,
            )
            .await
        };

        match result {
            Ok(()) => {
                info!("Session created successfully");
                // Reload workspaces BEFORE switching view to ensure UI shows new session immediately
                self.load_real_workspaces().await;

                // Start log streaming for the newly created session
                if let Err(e) = self.start_log_streaming_for_session(session_id).await {
                    warn!(
                        "Failed to start log streaming for session {}: {}",
                        session_id, e
                    );
                }

                // Force UI refresh to show new session immediately
                self.ui_needs_refresh = true;
                self.cancel_new_session();
            }
            Err(e) => {
                error!("Failed to create session: {}", e);
                self.cancel_new_session();
            }
        }
    }

    async fn create_restart_session_with_logs(
        &mut self,
        repo_path: &std::path::Path,
        branch_name: &str,
        session_id: Uuid,
        skip_permissions: bool,
        mode: crate::models::SessionMode,
        boss_prompt: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::docker::session_lifecycle::{SessionLifecycleManager, SessionRequest};
        use std::path::PathBuf;

        info!(
            "Creating restart session {} with updated configuration",
            session_id
        );

        // Create a channel for build logs
        let (log_sender, mut log_receiver) = mpsc::unbounded_channel::<String>();

        // Initialize logs for this session
        self.logs.insert(
            session_id,
            vec!["Restarting session with updated configuration...".to_string()],
        );

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
                info!(
                    "Restart log for session {}: {}",
                    session_id_clone, log_message
                );
            }
        });

        let workspace_name =
            repo_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();

        let request = SessionRequest {
            session_id,
            workspace_name,
            workspace_path: repo_path.to_path_buf(),
            branch_name: branch_name.to_string(),
            base_branch: None,
            container_config: None,
            skip_permissions,
            mode,
            boss_prompt,
        };

        // Add initial log message
        if let Some(session_logs) = self.logs.get_mut(&session_id) {
            session_logs.push("Checking for existing worktree...".to_string());
        }

        let mut manager = SessionLifecycleManager::new().await?;

        // Check if worktree exists from the previous session
        let existing_worktree_path = self
            .workspaces
            .iter()
            .flat_map(|w| &w.sessions)
            .find(|s| s.id == session_id)
            .map(|s| PathBuf::from(&s.workspace_path));

        let result = if let Some(worktree_path) = existing_worktree_path {
            if worktree_path.exists() {
                info!(
                    "Found existing worktree at {}, reusing it",
                    worktree_path.display()
                );

                if let Some(logs) = self.logs.get_mut(&session_id) {
                    logs.push(format!(
                        "Reusing existing worktree at {}",
                        worktree_path.display()
                    ));
                }

                let worktree_info = crate::git::WorktreeInfo {
                    id: session_id, // Use session ID as worktree ID
                    path: worktree_path.clone(),
                    session_path: worktree_path.clone(), // Same as path for existing worktrees
                    branch_name: branch_name.to_string(),
                    source_repository: repo_path.to_path_buf(),
                    commit_hash: None, // We don't track this for existing worktrees
                };

                manager.create_session_with_existing_worktree(request, worktree_info).await
            } else {
                info!("Worktree path no longer exists, creating fresh session");

                if let Some(logs) = self.logs.get_mut(&session_id) {
                    logs.push("Worktree not found, creating fresh session...".to_string());
                }

                manager.create_session_with_logs(request, Some(log_sender.clone())).await
            }
        } else {
            info!("No existing worktree info found, creating fresh session");

            if let Some(logs) = self.logs.get_mut(&session_id) {
                logs.push("Creating fresh session...".to_string());
            }

            manager.create_session_with_logs(request, Some(log_sender.clone())).await
        };

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
                Ok(_) => logs
                    .push("Session restarted successfully with updated configuration!".to_string()),
                Err(e) => logs.push(format!("Session restart failed: {}", e)),
            }
        }

        result.map(|_| ())?;
        Ok(())
    }

    async fn create_session_with_logs(
        &mut self,
        repo_path: &std::path::Path,
        branch_name: &str,
        session_id: Uuid,
        skip_permissions: bool,
        mode: crate::models::SessionMode,
        boss_prompt: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
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
                info!(
                    "Build log for session {}: {}",
                    session_id_clone, log_message
                );
            }
        });

        let workspace_name =
            repo_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();

        let request = SessionRequest {
            session_id,
            workspace_name,
            workspace_path: repo_path.to_path_buf(),
            branch_name: branch_name.to_string(),
            base_branch: None,
            container_config: None,
            skip_permissions,
            mode,
            boss_prompt,
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

    /// Clean up orphaned containers (containers without worktrees)
    pub async fn cleanup_orphaned_containers(&mut self) -> anyhow::Result<usize> {
        use crate::docker::ContainerManager;

        info!("Starting cleanup of orphaned containers");

        let container_manager = ContainerManager::new().await?;
        let containers = container_manager.list_claude_containers().await?;

        let mut cleaned_up = 0;

        for container in containers {
            if let Some(session_id_str) =
                container.labels.as_ref().and_then(|labels| labels.get("claude-session-id"))
            {
                if let Ok(session_id) = uuid::Uuid::parse_str(session_id_str) {
                    // Check if worktree exists for this session
                    let worktree_manager = crate::git::WorktreeManager::new()?;
                    match worktree_manager.get_worktree_info(session_id) {
                        Ok(_) => {
                            // Worktree exists, container is not orphaned
                            continue;
                        }
                        Err(_) => {
                            // Worktree missing, this is an orphaned container
                            info!(
                                "Found orphaned container for session {}, removing it",
                                session_id
                            );

                            if let Some(container_id) = &container.id {
                                // Remove the orphaned container (this will stop it first)
                                if let Err(e) =
                                    container_manager.remove_container_by_id(container_id).await
                                {
                                    warn!(
                                        "Failed to remove orphaned container {}: {}",
                                        container_id, e
                                    );
                                } else {
                                    cleaned_up += 1;
                                    info!(
                                        "Successfully removed orphaned container {}",
                                        container_id
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        if cleaned_up > 0 {
            info!("Cleaned up {} orphaned containers", cleaned_up);
            self.add_success_notification(format!(
                "🧹 Cleaned up {} orphaned containers",
                cleaned_up
            ));

            // Reload workspaces to reflect changes
            self.load_real_workspaces().await;
            self.ui_needs_refresh = true;
        } else {
            info!("No orphaned containers found");
            self.add_info_notification("✅ No orphaned containers found".to_string());
        }

        Ok(cleaned_up)
    }

    async fn delete_session(&mut self, session_id: Uuid) -> anyhow::Result<()> {
        use crate::docker::{ContainerManager, SessionLifecycleManager};
        use crate::git::WorktreeManager;

        info!("Deleting session: {}", session_id);

        // Log workspace count before deletion
        let workspace_count_before = self.workspaces.len();
        let session_count_before: usize = self.workspaces.iter().map(|w| w.sessions.len()).sum();
        info!(
            "Before deletion: {} workspaces, {} sessions",
            workspace_count_before, session_count_before
        );

        // First, try to find and remove the container directly
        // This ensures we clean up containers even if they're not in the lifecycle manager
        let container_name = format!("claude-session-{}", session_id);
        let container_manager = ContainerManager::new().await?;

        info!("Looking for container: {}", container_name);
        if let Ok(containers) = container_manager.list_claude_containers().await {
            for container in containers {
                if let Some(names) = &container.names {
                    if names.iter().any(|n| n.trim_start_matches('/') == container_name) {
                        info!("Found container for session {}, removing it", session_id);
                        if let Some(container_id) = &container.id {
                            match container_manager.remove_container_by_id(container_id).await {
                                Ok(_) => info!("Successfully removed container {}", container_id),
                                Err(e) => {
                                    warn!("Failed to remove container {}: {}", container_id, e)
                                }
                            }
                        }
                        break;
                    }
                }
            }
        }

        // Create session lifecycle manager
        let mut manager = SessionLifecycleManager::new().await?;

        // Try to remove the session through lifecycle manager (this will handle worktree)
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
        info!(
            "After deletion: {} workspaces, {} sessions",
            workspace_count_after, session_count_after
        );

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
                    use tokio::time::{Duration, timeout};
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
                AsyncAction::NewSessionNormal => {
                    self.new_session_normal().await;
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
                        warn!(
                            "Failed to fetch container logs for session {}: {}",
                            session_id, e
                        );
                    }
                    self.ui_needs_refresh = true;
                }
                AsyncAction::AttachToContainer(session_id) => {
                    info!("Attaching to container for session {}", session_id);
                    if let Err(e) = self.attach_to_container(session_id).await {
                        error!(
                            "Failed to attach to container for session {}: {}",
                            session_id, e
                        );
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
                            auth_state.error_message =
                                Some(format!("Failed to save API key: {}", e));
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
                AsyncAction::RestartSession(session_id) => {
                    info!("Starting session restart for session {}", session_id);
                    if let Err(e) = self.handle_restart_session(session_id).await {
                        error!("Failed to restart session: {}", e);
                    }
                }
                AsyncAction::CleanupOrphaned => {
                    info!("Starting cleanup of orphaned containers");
                    if let Err(e) = self.cleanup_orphaned_containers().await {
                        error!("Failed to cleanup orphaned containers: {}", e);
                        self.add_error_notification(format!(
                            "❌ Failed to cleanup orphaned containers: {}",
                            e
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Run OAuth authentication setup
    async fn run_oauth_setup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use crossterm::{
            execute,
            terminal::{LeaveAlternateScreen, disable_raw_mode},
        };

        // Create auth directory
        let home_dir = dirs::home_dir().ok_or("Could not determine home directory")?;
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
                     Please start Docker and try again."
                        .to_string(),
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
                         Please check Docker and try again."
                            .to_string(),
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
        let success =
            status.success() && credentials_path.exists() && credentials_path.metadata()?.len() > 0;

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
                     Please try again or use API Key method."
                        .to_string(),
                );
                auth_state.is_processing = false;
            }
        }

        // Re-enable raw mode and return to TUI
        use crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};
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
        let home_dir = dirs::home_dir().ok_or("Could not determine home directory")?;
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
        let running_session_count =
            self.workspaces.iter().map(|w| w.running_sessions().len()).sum::<usize>();

        if running_session_count > 0 {
            warn!(
                "Found {} running sessions - re-authentication will affect them",
                running_session_count
            );

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
        let home_dir = dirs::home_dir().ok_or("Could not determine home directory")?;
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
            error_message: Some(
                "🔄 Previous credentials cleared - please authenticate again".to_string(),
            ),
        });
        self.current_view = View::AuthSetup;

        info!("Re-authentication initiated - switched to auth setup view");
        Ok(())
    }

    async fn handle_restart_session(
        &mut self,
        session_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Initiating restart UI flow for session {}", session_id);

        // Find the session in our workspace list
        let session_info = self.workspaces.iter().find_map(|workspace| {
            workspace
                .sessions
                .iter()
                .find(|s| s.id == session_id)
                .map(|session| (workspace, session))
        });

        if let Some((workspace, session)) = session_info {
            match &session.status {
                crate::models::SessionStatus::Stopped => {
                    info!(
                        "Session {} is stopped, starting restart UI flow",
                        session_id
                    );

                    // Start the new session UI flow with pre-populated data from the existing session
                    self.current_view = View::NewSession;
                    self.new_session_state = Some(NewSessionState {
                        available_repos: vec![workspace.path.clone()],
                        filtered_repos: vec![(0, workspace.path.clone())],
                        selected_repo_index: Some(0),
                        branch_name: session.branch_name.clone(),
                        step: NewSessionStep::InputBranch, // Start at branch input since repo is pre-selected
                        filter_text: String::new(),
                        is_current_dir_mode: false,
                        skip_permissions: session.skip_permissions,
                        mode: session.mode.clone(),
                        boss_prompt: if let Some(ref prompt) = session.boss_prompt {
                            TextEditor::from_string(prompt)
                        } else {
                            TextEditor::new()
                        },
                        file_finder: FuzzyFileFinderState::new(),
                        restart_session_id: Some(session_id), // Mark this as a restart operation
                    });

                    self.add_info_notification(
                        "🔄 Restarting session - review and update settings as needed".to_string(),
                    );
                }
                status => {
                    warn!(
                        "Cannot restart session {} - current status: {:?}",
                        session_id, status
                    );
                    self.add_error_notification(format!(
                        "❌ Cannot restart session - current status: {:?}",
                        status
                    ));
                }
            }
        } else {
            error!("Session {} not found in workspaces", session_id);
            self.add_error_notification("❌ Session not found".to_string());
        }

        Ok(())
    }

    pub fn show_git_view(&mut self) {
        // Get the selected session's workspace path
        if let Some(session) = self.get_selected_session() {
            let worktree_path = std::path::PathBuf::from(&session.workspace_path);
            let mut git_state = crate::components::GitViewState::new(worktree_path);

            // Refresh git status
            if let Err(e) = git_state.refresh_git_status() {
                tracing::error!("Failed to refresh git status: {}", e);
                return;
            }

            self.git_view_state = Some(git_state);
            self.current_view = View::GitView;
        } else {
            tracing::warn!("No session selected for git view");
        }
    }

    pub fn git_commit_and_push(&mut self) {
        let result = if let Some(git_state) = self.git_view_state.as_mut() {
            git_state.commit_and_push()
        } else {
            return;
        };

        match result {
            Ok(message) => {
                tracing::info!("Git commit and push successful: {}", message);
                // Set pending event to be processed in next loop iteration
                self.pending_event = Some(crate::app::events::AppEvent::GitCommitSuccess(message));
                // Refresh git status after successful push
                if let Some(git_state) = self.git_view_state.as_mut() {
                    if let Err(e) = git_state.refresh_git_status() {
                        tracing::error!("Failed to refresh git status after push: {}", e);
                        self.add_warning_notification(
                            "⚠️ Push successful but failed to refresh git status".to_string(),
                        );
                    }
                }
            }
            Err(e) => {
                tracing::error!("Git commit and push failed: {}", e);
                self.add_error_notification(format!("❌ Git push failed: {}", e));
            }
        }
    }

    // Quick commit dialog methods
    pub fn is_in_quick_commit_mode(&self) -> bool {
        self.quick_commit_message.is_some()
    }

    pub fn start_quick_commit(&mut self) {
        // Only start quick commit if we have a selected session and it's in a git repository
        if let Some(session) = self.get_selected_session() {
            // Check if the workspace path is a git repository
            let workspace_path = std::path::Path::new(&session.workspace_path);
            let git_dir = workspace_path.join(".git");

            if git_dir.exists() {
                self.quick_commit_message = Some(String::new());
                self.quick_commit_cursor = 0;
                self.add_info_notification(
                    "📝 Enter commit message and press Enter to commit & push".to_string(),
                );
            } else {
                self.add_warning_notification(
                    "⚠️ Selected workspace is not a git repository".to_string(),
                );
            }
        } else {
            self.add_warning_notification("⚠️ No session selected".to_string());
        }
    }

    pub fn cancel_quick_commit(&mut self) {
        self.quick_commit_message = None;
        self.quick_commit_cursor = 0;
        self.add_info_notification("❌ Quick commit cancelled".to_string());
    }

    pub fn add_char_to_quick_commit(&mut self, ch: char) {
        if let Some(ref mut message) = self.quick_commit_message {
            message.insert(self.quick_commit_cursor, ch);
            self.quick_commit_cursor += 1;
        }
    }

    pub fn backspace_quick_commit(&mut self) {
        if let Some(ref mut message) = self.quick_commit_message {
            if self.quick_commit_cursor > 0 {
                self.quick_commit_cursor -= 1;
                message.remove(self.quick_commit_cursor);
            }
        }
    }

    pub fn move_quick_commit_cursor_left(&mut self) {
        if self.quick_commit_cursor > 0 {
            self.quick_commit_cursor -= 1;
        }
    }

    pub fn move_quick_commit_cursor_right(&mut self) {
        if let Some(ref message) = self.quick_commit_message {
            if self.quick_commit_cursor < message.len() {
                self.quick_commit_cursor += 1;
            }
        }
    }

    pub fn confirm_quick_commit(&mut self) {
        if let Some(ref message) = self.quick_commit_message {
            if message.trim().is_empty() {
                self.add_warning_notification("⚠️ Commit message cannot be empty".to_string());
                return;
            }

            // Perform the quick commit
            self.perform_quick_commit(message.trim().to_string());
        }
    }

    fn perform_quick_commit(&mut self, commit_message: String) {
        let worktree_path = if let Some(session) = self.get_selected_session() {
            std::path::PathBuf::from(&session.workspace_path)
        } else {
            return;
        };

        // Use the shared git operations function - DRY compliance!
        match crate::git::operations::commit_and_push_changes(&worktree_path, &commit_message) {
            Ok(success_message) => {
                tracing::info!("Quick commit successful: {}", success_message);
                // Set pending event to be processed in next loop iteration
                self.pending_event = Some(crate::app::events::AppEvent::GitCommitSuccess(
                    success_message,
                ));
                // Clear quick commit state
                self.quick_commit_message = None;
                self.quick_commit_cursor = 0;
            }
            Err(e) => {
                tracing::error!("Quick commit failed: {}", e);
                self.add_error_notification(format!("❌ Quick commit failed: {}", e));
                // Keep quick commit dialog open so user can try again
            }
        }
    }

    /// Add a notification to the notification queue
    pub fn add_notification(&mut self, notification: Notification) {
        self.notifications.push(notification);
    }

    /// Add a success notification
    pub fn add_success_notification(&mut self, message: String) {
        self.add_notification(Notification::success(message));
    }

    /// Add an error notification
    pub fn add_error_notification(&mut self, message: String) {
        self.add_notification(Notification::error(message));
    }

    /// Add an info notification
    pub fn add_info_notification(&mut self, message: String) {
        self.add_notification(Notification::info(message));
    }

    /// Add a warning notification
    pub fn add_warning_notification(&mut self, message: String) {
        self.add_notification(Notification::warning(message));
    }

    /// Remove expired notifications
    pub fn cleanup_expired_notifications(&mut self) {
        self.notifications.retain(|n| !n.is_expired());
    }

    /// Get current notifications (non-expired)
    pub fn get_current_notifications(&self) -> Vec<&Notification> {
        self.notifications.iter().filter(|n| !n.is_expired()).collect()
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
        // Initialize log streaming coordinator
        let (mut coordinator, log_sender) = LogStreamingCoordinator::new();

        // Initialize the streaming manager inside the coordinator
        if let Err(e) = coordinator.init_manager(log_sender.clone()) {
            warn!("Failed to initialize log streaming manager: {}", e);
        } else {
            info!("Log streaming coordinator initialized successfully");
        }

        self.state.log_streaming_coordinator = Some(coordinator);
        self.state.log_sender = Some(log_sender);

        // Try to refresh OAuth tokens if they're expired (before checking first-time setup)
        let home_dir = dirs::home_dir();
        if let Some(home) = home_dir {
            let credentials_path = home.join(".claude-in-a-box").join("auth").join(".credentials.json");

            // Only attempt refresh if we have OAuth credentials that need refreshing
            if credentials_path.exists() && AppState::oauth_token_needs_refresh(&credentials_path) {
                info!("OAuth token needs refresh on startup, attempting automatic refresh");
                match self.state.refresh_oauth_tokens().await {
                    Ok(()) => info!("OAuth tokens refreshed successfully on startup"),
                    Err(e) => warn!("Failed to refresh OAuth tokens on startup: {}", e),
                }
            }
        }

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
            // Initialize Claude integration
            if let Err(e) = self.state.init_claude_integration().await {
                warn!("Failed to initialize Claude integration: {}", e);
            }

            self.state.check_current_directory_status();
            self.state.load_real_workspaces().await;

            // Start log streaming for any running sessions
            if let Err(e) = self.init_log_streaming_for_sessions().await {
                warn!(
                    "Failed to initialize log streaming for existing sessions: {}",
                    e
                );
            }
        }
    }

    /// Initialize log streaming for all running sessions
    async fn init_log_streaming_for_sessions(&mut self) -> anyhow::Result<()> {
        if let Some(coordinator) = &mut self.state.log_streaming_coordinator {
            // Collect session info for streaming
            let sessions: Vec<(Uuid, String, String, crate::models::SessionMode)> = self
                .state
                .workspaces
                .iter()
                .flat_map(|w| &w.sessions)
                .filter(|s| s.status == crate::models::SessionStatus::Running)
                .filter_map(|s| {
                    s.container_id.clone().map(|container_id| {
                        (
                            s.id,
                            container_id,
                            format!("{}-{}", s.name, s.branch_name),
                            s.mode.clone(),
                        )
                    })
                })
                .collect();

            if !sessions.is_empty() {
                info!(
                    "Starting log streaming for {} running sessions",
                    sessions.len()
                );
                for (session_id, container_id, container_name, session_mode) in &sessions {
                    if let Err(e) = coordinator
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
            }
        }
        Ok(())
    }

    pub async fn tick(&mut self) -> anyhow::Result<()> {
        // Clean up expired notifications
        self.state.cleanup_expired_notifications();

        // Periodic OAuth token refresh check (every 5 minutes)
        let now = Instant::now();
        let should_check_token = self
            .state
            .last_token_refresh_check
            .map(|last| now.duration_since(last).as_secs() >= 300) // Check every 5 minutes
            .unwrap_or(true); // First time

        if should_check_token {
            self.state.last_token_refresh_check = Some(now);

            // Check if we need to refresh OAuth tokens
            let home_dir = dirs::home_dir();
            if let Some(home) = home_dir {
                let credentials_path = home.join(".claude-in-a-box").join("auth").join(".credentials.json");

                if credentials_path.exists() && AppState::oauth_token_needs_refresh(&credentials_path) {
                    info!("OAuth token needs refresh (periodic check)");

                    // Refresh tokens inline (this is quick enough not to block UI)
                    match self.state.refresh_oauth_tokens().await {
                        Ok(()) => {
                            info!("OAuth tokens refreshed successfully (periodic)");
                            // Add a notification to inform the user
                            self.state.add_notification(Notification {
                                message: "✅ OAuth tokens refreshed automatically".to_string(),
                                notification_type: NotificationType::Success,
                                created_at: Instant::now(),
                                duration: Duration::from_secs(5),
                            });
                        }
                        Err(e) => {
                            warn!("Failed to refresh OAuth tokens (periodic): {}", e);
                            // Add a warning notification
                            self.state.add_notification(Notification {
                                message: format!("⚠️ Token refresh failed: {}", e),
                                notification_type: NotificationType::Warning,
                                created_at: Instant::now(),
                                duration: Duration::from_secs(10),
                            });
                        }
                    }
                }
            }
        }

        // Process incoming log entries (non-blocking)
        let mut log_entries = Vec::new();
        if let Some(coordinator) = &mut self.state.log_streaming_coordinator {
            // Collect all available log entries without blocking
            while let Some((session_id, log_entry)) = coordinator.try_next_log() {
                log_entries.push((session_id, log_entry));
            }
        }

        // Add log entries to the state
        for (session_id, log_entry) in log_entries {
            self.state.add_live_log(session_id, log_entry);
        }

        // Process any pending async actions
        match self.state.process_async_action().await {
            Ok(()) => {}
            Err(e) => {
                warn!("Error processing async action: {}", e);
                // Return to safe state if there was an error
                self.state.new_session_state = None;
                self.state.current_view = View::SessionList;
                self.state.pending_async_action = None;
            }
        }

        // Update logic for the app (e.g., refresh container status)

        // Periodic log updates for attached sessions
        let now = Instant::now();
        let should_update_logs = self
            .state
            .last_log_check
            .map(|last| now.duration_since(last).as_secs() >= 3) // Update every 3 seconds
            .unwrap_or(true); // First time

        if should_update_logs {
            self.state.last_log_check = Some(now);

            // If we have an attached session, fetch its logs
            if let Some(attached_id) = self.state.attached_session_id {
                // Check if we should update this session's logs (don't spam updates)
                let should_update_session = self
                    .state
                    .log_last_updated
                    .get(&attached_id)
                    .map(|last| now.duration_since(*last).as_secs() >= 2) // Update session logs every 2 seconds
                    .unwrap_or(true);

                if should_update_session {
                    // Fetch logs in the background (don't block the UI)
                    if let Err(e) = self.state.fetch_claude_logs(attached_id).await {
                        warn!("Failed to fetch logs for session {}: {}", attached_id, e);
                    } else {
                        self.state.log_last_updated.insert(attached_id, now);
                        // Set flag to refresh UI with new logs
                        self.state.ui_needs_refresh = true;
                    }
                }
            }
        }

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

// Include the test module inline
#[cfg(test)]
#[path = "state_tests.rs"]
mod state_tests;
