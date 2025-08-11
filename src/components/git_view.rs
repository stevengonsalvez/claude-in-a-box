// ABOUTME: Git view component for displaying git status, changed files, and diffs with commit/push functionality

use anyhow::Result;
use git2::{DiffFormat, DiffOptions, Repository};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap},
};
use std::path::PathBuf;
use tracing::{debug, error};

#[derive(Debug, Clone)]
pub struct GitViewState {
    pub active_tab: GitTab,
    pub changed_files: Vec<ChangedFile>,
    pub selected_file_index: usize,
    pub diff_content: Vec<String>,
    pub diff_scroll_offset: usize,
    pub worktree_path: PathBuf,
    pub is_dirty: bool,
    pub can_push: bool,
    pub commit_message_input: Option<String>, // None = not in commit mode, Some = commit message being entered
    pub commit_message_cursor: usize,         // Cursor position in commit message
}

#[derive(Debug, Clone, PartialEq)]
pub enum GitTab {
    Files,
    Diff,
}

#[derive(Debug, Clone)]
pub struct ChangedFile {
    pub path: String,
    pub status: GitFileStatus,
    pub insertions: usize,
    pub deletions: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GitFileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Untracked,
}

impl GitFileStatus {
    pub fn symbol(&self) -> &'static str {
        match self {
            GitFileStatus::Added => "A",
            GitFileStatus::Modified => "M",
            GitFileStatus::Deleted => "D",
            GitFileStatus::Renamed => "R",
            GitFileStatus::Untracked => "?",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            GitFileStatus::Added => Color::Green,
            GitFileStatus::Modified => Color::Yellow,
            GitFileStatus::Deleted => Color::Red,
            GitFileStatus::Renamed => Color::Blue,
            GitFileStatus::Untracked => Color::Magenta,
        }
    }
}

impl GitViewState {
    pub fn new(worktree_path: PathBuf) -> Self {
        Self {
            active_tab: GitTab::Files,
            changed_files: Vec::new(),
            selected_file_index: 0,
            diff_content: Vec::new(),
            diff_scroll_offset: 0,
            worktree_path,
            is_dirty: false,
            can_push: false,
            commit_message_input: None,
            commit_message_cursor: 0,
        }
    }

    pub fn refresh_git_status(&mut self) -> Result<()> {
        debug!(
            "Refreshing git status for worktree: {:?}",
            self.worktree_path
        );

        let repo = Repository::open(&self.worktree_path)?;
        let mut changed_files = Vec::new();

        // Get working directory changes
        let mut opts = DiffOptions::new();
        opts.include_untracked(true);
        opts.include_ignored(false);

        let diff = repo.diff_index_to_workdir(None, Some(&mut opts))?;

        diff.foreach(
            &mut |delta, _progress| {
                if let Some(new_file) = delta.new_file().path() {
                    let path = new_file.to_string_lossy().to_string();
                    let status = match delta.status() {
                        git2::Delta::Added => GitFileStatus::Added,
                        git2::Delta::Modified => GitFileStatus::Modified,
                        git2::Delta::Deleted => GitFileStatus::Deleted,
                        git2::Delta::Renamed => GitFileStatus::Renamed,
                        git2::Delta::Untracked => GitFileStatus::Untracked,
                        _ => GitFileStatus::Modified,
                    };

                    changed_files.push(ChangedFile {
                        path,
                        status,
                        insertions: 0, // Will be calculated in line callback
                        deletions: 0,
                    });
                }
                true
            },
            None,
            None,
            None,
        )?;

        // Check if there are staged changes
        let head_tree = repo.head()?.peel_to_tree()?;
        let staged_diff = repo.diff_tree_to_index(Some(&head_tree), None, None)?;
        let has_staged_changes = staged_diff.deltas().len() > 0;

        self.changed_files = changed_files;
        self.is_dirty = !self.changed_files.is_empty() || has_staged_changes;

        // Check if we can push (has commits ahead of remote)
        self.can_push = self.check_can_push(&repo)?;

        // Reset selection if needed
        if self.selected_file_index >= self.changed_files.len() && !self.changed_files.is_empty() {
            self.selected_file_index = 0;
        }

        // Refresh diff for selected file
        if !self.changed_files.is_empty() {
            self.refresh_diff_for_selected_file()?;
        } else {
            self.diff_content.clear();
        }

        Ok(())
    }

    fn check_can_push(&self, repo: &Repository) -> Result<bool> {
        // Check if there are commits ahead of the remote
        match repo.head() {
            Ok(head_ref) => {
                let head_oid = match head_ref.target() {
                    Some(oid) => oid,
                    None => return Ok(false), // Symbolic ref pointing to nothing
                };

                // Try to find the upstream branch
                let branch_name = head_ref.shorthand().unwrap_or("HEAD");
                let upstream_name = format!("origin/{}", branch_name);

                match repo.revparse_single(&upstream_name) {
                    Ok(upstream_commit) => {
                        let upstream_oid = upstream_commit.id();

                        // Check if head is ahead of upstream
                        let (ahead, _behind) = repo.graph_ahead_behind(head_oid, upstream_oid)?;
                        Ok(ahead > 0)
                    }
                    Err(_) => {
                        // No upstream, can push if there are commits
                        Ok(true)
                    }
                }
            }
            Err(_) => Ok(false),
        }
    }

    pub fn refresh_diff_for_selected_file(&mut self) -> Result<()> {
        if self.changed_files.is_empty() {
            self.diff_content.clear();
            return Ok(());
        }

        let selected_file = &self.changed_files[self.selected_file_index];
        debug!("Refreshing diff for file: {}", selected_file.path);

        let repo = Repository::open(&self.worktree_path)?;
        let mut diff_content = Vec::new();

        // Create diff options
        let mut opts = DiffOptions::new();
        opts.pathspec(&selected_file.path);

        let diff = match selected_file.status {
            GitFileStatus::Untracked => {
                // For untracked files, show the entire file content as additions
                let file_path = self.worktree_path.join(&selected_file.path);
                match std::fs::read_to_string(&file_path) {
                    Ok(content) => {
                        diff_content.push(format!("--- /dev/null"));
                        diff_content.push(format!("+++ b/{}", selected_file.path));
                        diff_content.push(format!("@@ -0,0 +1,{} @@", content.lines().count()));
                        for line in content.lines() {
                            diff_content.push(format!("+{}", line));
                        }
                    }
                    Err(e) => {
                        diff_content.push(format!("Error reading file: {}", e));
                    }
                }
                self.diff_content = diff_content;
                return Ok(());
            }
            _ => repo.diff_index_to_workdir(None, Some(&mut opts))?,
        };

        // Format the diff
        diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
            let content = std::str::from_utf8(line.content()).unwrap_or("<binary>");
            let line_str = match line.origin() {
                '+' => format!("+{}", content.trim_end()),
                '-' => format!("-{}", content.trim_end()),
                ' ' => format!(" {}", content.trim_end()),
                '=' => format!("={}", content.trim_end()),
                '>' => format!(">{}", content.trim_end()),
                '<' => format!("<{}", content.trim_end()),
                'F' => format!("File: {}", content.trim_end()),
                'H' => format!("Hunk: {}", content.trim_end()),
                _ => content.trim_end().to_string(),
            };
            diff_content.push(line_str);
            true
        })?;

        self.diff_content = diff_content;
        self.diff_scroll_offset = 0; // Reset scroll when changing files

        Ok(())
    }

    pub fn next_file(&mut self) {
        if !self.changed_files.is_empty() {
            self.selected_file_index = (self.selected_file_index + 1) % self.changed_files.len();
            if let Err(e) = self.refresh_diff_for_selected_file() {
                error!("Failed to refresh diff: {}", e);
            }
        }
    }

    pub fn previous_file(&mut self) {
        if !self.changed_files.is_empty() {
            self.selected_file_index = if self.selected_file_index == 0 {
                self.changed_files.len() - 1
            } else {
                self.selected_file_index - 1
            };
            if let Err(e) = self.refresh_diff_for_selected_file() {
                error!("Failed to refresh diff: {}", e);
            }
        }
    }

    pub fn scroll_diff_up(&mut self) {
        if self.diff_scroll_offset > 0 {
            self.diff_scroll_offset -= 1;
        }
    }

    pub fn scroll_diff_down(&mut self) {
        if self.diff_scroll_offset < self.diff_content.len().saturating_sub(1) {
            self.diff_scroll_offset += 1;
        }
    }

    pub fn switch_tab(&mut self) {
        self.active_tab = match self.active_tab {
            GitTab::Files => GitTab::Diff,
            GitTab::Diff => GitTab::Files,
        };
    }

    pub fn start_commit_message_input(&mut self) {
        self.commit_message_input = Some(String::new());
        self.commit_message_cursor = 0;
    }

    pub fn cancel_commit_message_input(&mut self) {
        self.commit_message_input = None;
        self.commit_message_cursor = 0;
    }

    pub fn is_in_commit_mode(&self) -> bool {
        self.commit_message_input.is_some()
    }

    pub fn add_char_to_commit_message(&mut self, ch: char) {
        if let Some(ref mut message) = self.commit_message_input {
            message.insert(self.commit_message_cursor, ch);
            self.commit_message_cursor += 1;
        }
    }

    pub fn backspace_commit_message(&mut self) {
        if let Some(ref mut message) = self.commit_message_input {
            if self.commit_message_cursor > 0 {
                self.commit_message_cursor -= 1;
                message.remove(self.commit_message_cursor);
            }
        }
    }

    pub fn move_commit_cursor_left(&mut self) {
        if self.commit_message_cursor > 0 {
            self.commit_message_cursor -= 1;
        }
    }

    pub fn move_commit_cursor_right(&mut self) {
        if let Some(ref message) = self.commit_message_input {
            if self.commit_message_cursor < message.len() {
                self.commit_message_cursor += 1;
            }
        }
    }

    pub fn commit_and_push(&mut self) -> Result<String> {
        debug!(
            "Committing and pushing changes for worktree: {:?}",
            self.worktree_path
        );

        // Get the commit message, or return error if not in commit mode
        let commit_message = match &self.commit_message_input {
            Some(message) if !message.trim().is_empty() => message.trim().to_string(),
            Some(_) => return Err(anyhow::anyhow!("Commit message cannot be empty")),
            None => {
                return Err(anyhow::anyhow!(
                    "Not in commit mode - press 'p' to start commit process"
                ));
            }
        };

        // Try CLI git first as experiment
        debug!("=== EXPERIMENT: Trying CLI git first ===");
        match self.commit_and_push_cli(&commit_message) {
            Ok(result) => {
                debug!("✓ CLI git succeeded!");
                return Ok(result);
            }
            Err(e) => {
                debug!("✗ CLI git failed: {}, falling back to git2", e);
            }
        }

        // Fallback to git2 implementation
        debug!("=== Falling back to git2 implementation ===");
        self.commit_and_push_git2(&commit_message)
    }

    fn commit_and_push_cli(&mut self, commit_message: &str) -> Result<String> {
        use std::process::Command;
        use std::time::Duration;

        debug!("Using CLI git for commit and push");

        // Change to worktree directory
        let original_dir = std::env::current_dir()?;
        std::env::set_current_dir(&self.worktree_path)?;

        let result = (|| -> Result<String> {
            // Stage all changes
            debug!("Staging all changes with 'git add .'");
            let add_output = Command::new("git")
                .args(&["add", "."])
                .env("GIT_TERMINAL_PROMPT", "0") // Disable interactive prompts
                .output()?;

            if !add_output.status.success() {
                let stderr = String::from_utf8_lossy(&add_output.stderr);
                return Err(anyhow::anyhow!("git add failed: {}", stderr));
            }

            // Commit with --no-gpg-sign to avoid hanging on GPG passphrase
            // and --no-verify to skip pre-commit hooks that might hang
            debug!("Committing with message: {}", commit_message);
            let commit_output = Command::new("git")
                .args(&[
                    "commit",
                    "--no-gpg-sign",
                    "--no-verify",
                    "-m",
                    commit_message,
                ])
                .env("GIT_TERMINAL_PROMPT", "0") // Disable interactive prompts
                .env("GIT_ASKPASS", "echo") // Provide dummy askpass to avoid hanging
                .output()?;

            if !commit_output.status.success() {
                let stderr = String::from_utf8_lossy(&commit_output.stderr);
                let stdout = String::from_utf8_lossy(&commit_output.stdout);
                debug!("git commit stderr: {}", stderr);
                debug!("git commit stdout: {}", stdout);
                return Err(anyhow::anyhow!("git commit failed: {}", stderr));
            }

            // Push
            debug!("Pushing to origin");
            let push_output = Command::new("git")
                .args(&["push", "origin", "HEAD"])
                .env("GIT_TERMINAL_PROMPT", "0") // Disable interactive prompts
                .env("GIT_ASKPASS", "echo") // Provide dummy askpass to avoid hanging
                .output()?;

            if !push_output.status.success() {
                let stderr = String::from_utf8_lossy(&push_output.stderr);
                let stdout = String::from_utf8_lossy(&push_output.stdout);
                error!("git push failed - stderr: {}", stderr);
                error!("git push failed - stdout: {}", stdout);
                return Err(anyhow::anyhow!("git push failed: {}", stderr));
            }

            debug!("CLI git push succeeded");
            Ok(format!("Committed and pushed: {}", commit_message))
        })();

        // Always restore original directory
        std::env::set_current_dir(original_dir)?;

        // Clear commit message input after successful commit
        if result.is_ok() {
            self.commit_message_input = None;
            self.commit_message_cursor = 0;
        }

        result
    }

    fn commit_and_push_git2(&mut self, commit_message: &str) -> Result<String> {
        let repo = Repository::open(&self.worktree_path)?;

        // Stage all changes
        let mut index = repo.index()?;

        // Add all changed files to the index
        for file in &self.changed_files {
            match file.status {
                GitFileStatus::Deleted => {
                    index.remove_path(std::path::Path::new(&file.path))?;
                }
                _ => {
                    index.add_path(std::path::Path::new(&file.path))?;
                }
            }
        }

        index.write()?;

        // Create commit
        let signature = repo.signature()?;
        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;

        let parent_commit = match repo.head() {
            Ok(head) => Some(head.peel_to_commit()?),
            Err(_) => None,
        };

        let parents: Vec<&git2::Commit> = parent_commit.iter().collect();

        let commit_id = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            &commit_message,
            &tree,
            &parents,
        )?;

        debug!("Created commit: {}", commit_id);

        // Push to remote using system's git authentication
        let mut remote = repo.find_remote("origin")?;
        let head_ref = repo.head()?;
        let branch_name = head_ref.shorthand().unwrap_or("HEAD");
        let refspec = format!("refs/heads/{}:refs/heads/{}", branch_name, branch_name);

        // Configure push options with credential callback to use system git credentials
        let mut push_options = git2::PushOptions::new();
        let mut remote_callbacks = git2::RemoteCallbacks::new();

        // Set up credential callback to use system git credential helpers
        remote_callbacks.credentials(|_url, username_from_url, _allowed_types| {
            debug!("=== Git credential callback triggered ===");
            debug!("URL: {}", _url);
            debug!("Username from URL: {:?}", username_from_url);
            debug!("Allowed types: {:?}", _allowed_types);

            // Try to use git credential helper (works with credential store, manager, etc.)
            debug!("Attempting git credential helper...");
            match git2::Cred::credential_helper(&repo.config()?, _url, username_from_url) {
                Ok(cred) => {
                    debug!("✓ Successfully got credentials from git credential helper");
                    return Ok(cred);
                }
                Err(e) => {
                    debug!("✗ Git credential helper failed: {}", e);
                }
            }

            // Fallback to default credential (for SSH keys, etc.)
            debug!("Attempting default git credentials...");
            match git2::Cred::default() {
                Ok(cred) => {
                    debug!("✓ Successfully got default git credentials");
                    return Ok(cred);
                }
                Err(e) => {
                    debug!("✗ Default git credentials failed: {}", e);
                }
            }

            // If all else fails, try username/password from URL
            if let Some(username) = username_from_url {
                debug!("Attempting username/password from URL: {}", username);
                match git2::Cred::userpass_plaintext(username, "") {
                    Ok(cred) => {
                        debug!("✓ Successfully created userpass credentials");
                        return Ok(cred);
                    }
                    Err(e) => {
                        debug!("✗ Userpass credentials failed: {}", e);
                    }
                }
            }

            debug!("✗ All credential methods failed");
            Err(git2::Error::from_str("No credentials available"))
        });

        push_options.remote_callbacks(remote_callbacks);

        // Push with credential support
        debug!("Attempting to push refspec: {}", refspec);
        match remote.push(&[&refspec], Some(&mut push_options)) {
            Ok(()) => {
                debug!("✓ Successfully pushed to remote");
            }
            Err(e) => {
                // Log detailed error information
                error!("=== Git push failed ===");
                error!("Error message: {}", e.message());
                error!("Error code: {:?}", e.code());
                error!("Error class: {:?}", e.class());
                error!("Refspec: {}", refspec);
                error!("Remote URL: {:?}", remote.url());

                // Check git config for debugging
                if let Ok(config) = repo.config() {
                    if let Ok(entries) = config.entries(Some("credential.*")) {
                        error!("Git credential config:");
                        let _ = entries.for_each(|entry| {
                            if let (Some(name), Some(value)) = (entry.name(), entry.value()) {
                                error!("  {} = {}", name, value);
                            }
                        });
                    }
                }

                // Provide user-friendly error messages based on the error content
                let error_msg = e.message();
                let user_friendly_msg = if error_msg.contains("authentication")
                    || error_msg.contains("Authentication")
                {
                    "Authentication failed. Please check your git credentials (SSH keys, tokens, etc.)"
                } else if error_msg.contains("network")
                    || error_msg.contains("Network")
                    || error_msg.contains("connection")
                {
                    "Network error. Please check your internet connection."
                } else if error_msg.contains("not found") || error_msg.contains("Not found") {
                    "Remote repository not found. Please check the remote URL."
                } else if error_msg.contains("permission") || error_msg.contains("Permission") {
                    "Permission denied. Please check your access rights to the repository."
                } else {
                    error_msg
                };
                return Err(anyhow::anyhow!("Push failed: {}", user_friendly_msg));
            }
        }

        // Clear commit message input after successful commit
        self.commit_message_input = None;
        self.commit_message_cursor = 0;

        Ok(format!("Committed and pushed: {}", commit_message))
    }
}

pub struct GitViewComponent;

impl GitViewComponent {
    pub fn render(frame: &mut Frame, area: Rect, git_state: &GitViewState) {
        // Create main layout - adjust constraints based on commit mode
        let constraints = if git_state.is_in_commit_mode() {
            vec![
                Constraint::Length(3), // Tabs
                Constraint::Min(0),    // Content
                Constraint::Length(5), // Commit message input
                Constraint::Length(3), // Status/Actions
            ]
        } else {
            vec![
                Constraint::Length(3), // Tabs
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Status/Actions
            ]
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        // Render tabs
        let tab_titles = vec!["Files", "Diff"];
        let selected_tab = match git_state.active_tab {
            GitTab::Files => 0,
            GitTab::Diff => 1,
        };

        let tabs = Tabs::new(tab_titles)
            .block(Block::default().borders(Borders::ALL).title("Git Status"))
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .select(selected_tab);

        frame.render_widget(tabs, chunks[0]);

        // Render content based on active tab
        match git_state.active_tab {
            GitTab::Files => Self::render_files_tab(frame, chunks[1], git_state),
            GitTab::Diff => Self::render_diff_tab(frame, chunks[1], git_state),
        }

        // Render commit message input if in commit mode
        if git_state.is_in_commit_mode() {
            Self::render_commit_input(frame, chunks[2], git_state);
            // Status bar is at index 3 when commit input is shown
            Self::render_status_bar(frame, chunks[3], git_state);
        } else {
            // Status bar is at index 2 when no commit input
            Self::render_status_bar(frame, chunks[2], git_state);
        }
    }

    fn render_files_tab(frame: &mut Frame, area: Rect, git_state: &GitViewState) {
        if git_state.changed_files.is_empty() {
            let no_changes = Paragraph::new("No changes detected")
                .block(Block::default().borders(Borders::ALL).title("Changed Files"))
                .style(Style::default().fg(Color::Gray))
                .wrap(Wrap { trim: true });
            frame.render_widget(no_changes, area);
            return;
        }

        let items: Vec<ListItem> = git_state
            .changed_files
            .iter()
            .enumerate()
            .map(|(i, file)| {
                let style = if i == git_state.selected_file_index {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else {
                    Style::default()
                };

                let status_span = Span::styled(
                    format!("[{}]", file.status.symbol()),
                    Style::default().fg(file.status.color()).add_modifier(Modifier::BOLD),
                );

                let path_span = Span::styled(&file.path, style);

                let changes_span = if file.insertions > 0 || file.deletions > 0 {
                    Span::styled(
                        format!(" (+{} -{}) ", file.insertions, file.deletions),
                        Style::default().fg(Color::Gray),
                    )
                } else {
                    Span::raw("")
                };

                ListItem::new(Line::from(vec![
                    status_span,
                    Span::raw(" "),
                    path_span,
                    changes_span,
                ]))
                .style(style)
            })
            .collect();

        let files_list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Changed Files"))
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::White));

        let mut list_state = ListState::default();
        list_state.select(Some(git_state.selected_file_index));

        frame.render_stateful_widget(files_list, area, &mut list_state);
    }

    fn render_diff_tab(frame: &mut Frame, area: Rect, git_state: &GitViewState) {
        if git_state.diff_content.is_empty() {
            let no_diff = Paragraph::new(
                "No diff available\nSelect a file in the Files tab to view its diff",
            )
            .block(Block::default().borders(Borders::ALL).title("Diff"))
            .style(Style::default().fg(Color::Gray))
            .wrap(Wrap { trim: true });
            frame.render_widget(no_diff, area);
            return;
        }

        // Calculate visible lines
        let content_height = area.height.saturating_sub(2) as usize; // Account for borders
        let start_line = git_state.diff_scroll_offset;
        let end_line = (start_line + content_height).min(git_state.diff_content.len());

        let visible_lines: Vec<Line> = git_state.diff_content[start_line..end_line]
            .iter()
            .map(|line| {
                let style = if line.starts_with('+') {
                    Style::default().fg(Color::Green)
                } else if line.starts_with('-') {
                    Style::default().fg(Color::Red)
                } else if line.starts_with("@@") {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else if line.starts_with("+++") || line.starts_with("---") {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                Line::from(Span::styled(line.clone(), style))
            })
            .collect();

        let selected_file_name = git_state
            .changed_files
            .get(git_state.selected_file_index)
            .map(|f| f.path.as_str())
            .unwrap_or("No file selected");

        let diff_paragraph = Paragraph::new(visible_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Diff: {}", selected_file_name)),
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(diff_paragraph, area);
    }

    fn render_commit_input(frame: &mut Frame, area: Rect, git_state: &GitViewState) {
        let empty_string = String::new();
        let commit_message = git_state.commit_message_input.as_ref().unwrap_or(&empty_string);

        // Create the input text with cursor
        let mut display_text = commit_message.clone();
        if git_state.commit_message_cursor <= display_text.len() {
            display_text.insert(git_state.commit_message_cursor, '|');
        }

        let input_paragraph = Paragraph::new(display_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Commit Message (Enter to commit, Esc to cancel)"),
            )
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: false });

        frame.render_widget(input_paragraph, area);
    }

    fn render_status_bar(frame: &mut Frame, area: Rect, git_state: &GitViewState) {
        let status_text = if git_state.is_dirty {
            format!("{} files changed", git_state.changed_files.len())
        } else {
            "Working directory clean".to_string()
        };

        let push_status = if git_state.can_push {
            " • Ready to push"
        } else {
            " • Up to date"
        };

        let help_text = if git_state.is_in_commit_mode() {
            " [Enter] commit & push • [Esc] cancel • [←/→] move cursor"
        } else {
            match git_state.active_tab {
                GitTab::Files => {
                    " [j/k] navigate • [Tab] switch tab • [p] commit & push • [Esc] back"
                }
                GitTab::Diff => " [j/k] scroll • [Tab] switch tab • [p] commit & push • [Esc] back",
            }
        };

        let status_line = format!("{}{}{}", status_text, push_status, help_text);

        let status_paragraph = Paragraph::new(status_line)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: true });

        frame.render_widget(status_paragraph, area);
    }
}
