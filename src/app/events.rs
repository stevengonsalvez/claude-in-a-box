// ABOUTME: Event handling system for keyboard input and app actions

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::app::{AppState, state::{AsyncAction, View, AuthMethod}};
use tracing::info;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Quit,
    NextSession,
    PreviousSession,
    NextWorkspace,
    PreviousWorkspace,
    ToggleHelp,
    RefreshWorkspaces,  // Manual refresh of workspace data
    NewSession,         // Create session in current directory
    SearchWorkspace,    // Search all workspaces
    AttachSession,
    DetachSession,
    KillContainer,
    ReauthenticateCredentials,
    DeleteSession,
    SwitchToLogs,
    SwitchToTerminal,
    GoToTop,
    GoToBottom,
    // New session creation events
    NewSessionCancel,
    NewSessionNextRepo,
    NewSessionPrevRepo,
    NewSessionConfirmRepo,
    NewSessionInputChar(char),
    NewSessionBackspace,
    NewSessionCreate,
    // Search workspace events
    SearchWorkspaceInputChar(char),
    SearchWorkspaceBackspace,
    // Confirmation dialog events
    ConfirmationToggle,     // Switch between Yes/No
    ConfirmationConfirm,    // Confirm action
    ConfirmationCancel,     // Cancel dialog
    // Auth setup events
    AuthSetupNext,          // Next auth method
    AuthSetupPrevious,      // Previous auth method
    AuthSetupSelect,        // Select current method
    AuthSetupCancel,        // Cancel auth setup (skip)
    AuthSetupInputChar(char), // Input character for API key
    AuthSetupBackspace,     // Backspace in API key input
    AuthSetupCheckStatus,   // Check authentication status
    AuthSetupRefresh,       // Manual refresh to check auth completion
    AuthSetupShowCommand,   // Show manual CLI command
}

pub struct EventHandler;

impl EventHandler {
    pub fn handle_key_event(key_event: KeyEvent, state: &mut AppState) -> Option<AppEvent> {
        use crate::app::state::View;
        
        // Handle confirmation dialog first (highest priority)
        if state.confirmation_dialog.is_some() {
            match key_event.code {
                KeyCode::Left | KeyCode::Right | KeyCode::Tab => {
                    return Some(AppEvent::ConfirmationToggle);
                },
                KeyCode::Enter => {
                    return Some(AppEvent::ConfirmationConfirm);
                },
                KeyCode::Esc => {
                    return Some(AppEvent::ConfirmationCancel);
                },
                _ => return None,
            }
        }
        
        if state.help_visible {
            match key_event.code {
                KeyCode::Char('?') | KeyCode::Esc => {
                    return Some(AppEvent::ToggleHelp);
                },
                _ => {
                    return None;
                }
            }
        }

        // Handle global help toggle first (should work from any view)
        if let KeyCode::Char('?') = key_event.code {
            return Some(AppEvent::ToggleHelp);
        }

        // Handle new session creation view
        if state.current_view == View::NewSession {
            return Self::handle_new_session_keys(key_event, state);
        }

        // Handle search workspace view
        if state.current_view == View::SearchWorkspace {
            return Self::handle_search_workspace_keys(key_event, state);
        }

        // Handle non-git notification view
        if state.current_view == View::NonGitNotification {
            return Self::handle_non_git_notification_keys(key_event, state);
        }

        // Handle attached terminal view
        if state.current_view == View::AttachedTerminal {
            return Self::handle_attached_terminal_keys(key_event, state);
        }
        
        // Handle auth setup view
        if state.current_view == View::AuthSetup {
            return Self::handle_auth_setup_keys(key_event, state);
        }

        match key_event.code {
            KeyCode::Char('q') | KeyCode::Esc => Some(AppEvent::Quit),
            KeyCode::Char('j') | KeyCode::Down => Some(AppEvent::NextSession),
            KeyCode::Char('k') | KeyCode::Up => Some(AppEvent::PreviousSession),
            KeyCode::Char('h') | KeyCode::Left => Some(AppEvent::PreviousWorkspace),
            KeyCode::Char('l') | KeyCode::Right => Some(AppEvent::NextWorkspace),
            KeyCode::Char('g') => Some(AppEvent::GoToTop),
            KeyCode::Char('G') => Some(AppEvent::GoToBottom),
            KeyCode::Char('f') => Some(AppEvent::RefreshWorkspaces),  // Manual refresh
            KeyCode::Char('n') => Some(AppEvent::NewSession),
            KeyCode::Char('s') => Some(AppEvent::SearchWorkspace),
            KeyCode::Char('a') => Some(AppEvent::AttachSession),
            KeyCode::Char('r') => Some(AppEvent::ReauthenticateCredentials),  // Re-authenticate Claude credentials
            KeyCode::Char('d') => Some(AppEvent::DeleteSession),
            KeyCode::Tab => Some(AppEvent::SwitchToLogs),
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => Some(AppEvent::Quit),
            _ => None,
        }
    }

    fn handle_search_workspace_keys(key_event: KeyEvent, _state: &mut AppState) -> Option<AppEvent> {
        
        match key_event.code {
            KeyCode::Esc => {
                Some(AppEvent::NewSessionCancel)
            },
            KeyCode::Char('j') | KeyCode::Down => Some(AppEvent::NewSessionNextRepo),
            KeyCode::Char('k') | KeyCode::Up => Some(AppEvent::NewSessionPrevRepo),
            KeyCode::Enter => Some(AppEvent::NewSessionConfirmRepo),
            KeyCode::Backspace => Some(AppEvent::SearchWorkspaceBackspace),
            KeyCode::Char(ch) => Some(AppEvent::SearchWorkspaceInputChar(ch)),
            _ => None,
        }
    }

    fn handle_new_session_keys(key_event: KeyEvent, state: &mut AppState) -> Option<AppEvent> {
        use crate::app::state::NewSessionStep;
        
        if let Some(ref session_state) = state.new_session_state {
            match session_state.step {
                NewSessionStep::SelectRepo => {
                    match key_event.code {
                        KeyCode::Esc => Some(AppEvent::NewSessionCancel),
                        KeyCode::Char('j') | KeyCode::Down => Some(AppEvent::NewSessionNextRepo),
                        KeyCode::Char('k') | KeyCode::Up => Some(AppEvent::NewSessionPrevRepo),
                        KeyCode::Enter => Some(AppEvent::NewSessionConfirmRepo),
                        _ => None,
                    }
                }
                NewSessionStep::InputBranch => {
                    match key_event.code {
                        KeyCode::Esc => Some(AppEvent::NewSessionCancel),
                        KeyCode::Enter => Some(AppEvent::NewSessionCreate),
                        KeyCode::Backspace => Some(AppEvent::NewSessionBackspace),
                        KeyCode::Char(ch) => Some(AppEvent::NewSessionInputChar(ch)),
                        _ => None,
                    }
                }
                NewSessionStep::Creating => {
                    // During creation, only allow cancellation
                    match key_event.code {
                        KeyCode::Esc => Some(AppEvent::NewSessionCancel),
                        _ => None,
                    }
                }
            }
        } else {
            None
        }
    }

    fn handle_non_git_notification_keys(key_event: KeyEvent, _state: &mut AppState) -> Option<AppEvent> {
        match key_event.code {
            KeyCode::Char('q') | KeyCode::Esc => Some(AppEvent::Quit),
            KeyCode::Char('s') => Some(AppEvent::SearchWorkspace),
            _ => None,
        }
    }

    fn handle_attached_terminal_keys(key_event: KeyEvent, _state: &mut AppState) -> Option<AppEvent> {
        match key_event.code {
            KeyCode::Char('d') => Some(AppEvent::DetachSession),
            KeyCode::Char('q') | KeyCode::Esc => Some(AppEvent::DetachSession),
            KeyCode::Char('k') => Some(AppEvent::KillContainer),
            _ => None, // All other keys are passed through to the terminal
        }
    }
    
    fn handle_auth_setup_keys(key_event: KeyEvent, state: &mut AppState) -> Option<AppEvent> {
        if let Some(ref auth_state) = state.auth_setup_state {
            // If we're inputting API key, handle text input
            if auth_state.selected_method == AuthMethod::ApiKey && !auth_state.api_key_input.is_empty() {
                match key_event.code {
                    KeyCode::Enter => Some(AppEvent::AuthSetupSelect),
                    KeyCode::Backspace => Some(AppEvent::AuthSetupBackspace),
                    KeyCode::Esc => Some(AppEvent::AuthSetupBackspace), // Clear input
                    KeyCode::Char(ch) => Some(AppEvent::AuthSetupInputChar(ch)),
                    _ => None,
                }
            } else {
                // Method selection mode or waiting for auth completion
                match key_event.code {
                    KeyCode::Esc => Some(AppEvent::AuthSetupCancel),
                    KeyCode::Up | KeyCode::Char('k') => Some(AppEvent::AuthSetupPrevious),
                    KeyCode::Down | KeyCode::Char('j') => Some(AppEvent::AuthSetupNext),
                    KeyCode::Enter => Some(AppEvent::AuthSetupSelect),
                    KeyCode::Char('r') => Some(AppEvent::AuthSetupRefresh), // Manual refresh
                    KeyCode::Char('c') => Some(AppEvent::AuthSetupShowCommand), // Show CLI command
                    _ => None,
                }
            }
        } else {
            None
        }
    }

    pub fn process_event(event: AppEvent, state: &mut AppState) {
        match event {
            AppEvent::Quit => state.quit(),
            AppEvent::ToggleHelp => state.toggle_help(),
            AppEvent::RefreshWorkspaces => {
                // Mark for async processing to reload workspace data
                state.pending_async_action = Some(AsyncAction::RefreshWorkspaces);
            },
            AppEvent::NextSession => state.next_session(),
            AppEvent::PreviousSession => state.previous_session(),
            AppEvent::NextWorkspace => state.next_workspace(),
            AppEvent::PreviousWorkspace => state.previous_workspace(),
            AppEvent::GoToTop => {
                if state.selected_workspace_index.is_some() {
                    state.selected_session_index = Some(0);
                }
            },
            AppEvent::GoToBottom => {
                if let Some(workspace_idx) = state.selected_workspace_index {
                    if let Some(workspace) = state.workspaces.get(workspace_idx) {
                        if !workspace.sessions.is_empty() {
                            state.selected_session_index = Some(workspace.sessions.len() - 1);
                        }
                    }
                }
            },
            AppEvent::NewSession => {
                // Mark for async processing - create session in current directory
                state.pending_async_action = Some(AsyncAction::NewSessionInCurrentDir);
            },
            AppEvent::SearchWorkspace => {
                // Mark for async processing - search all workspaces
                state.pending_async_action = Some(AsyncAction::StartWorkspaceSearch);
                // Clear any previous cancellation flag
                state.async_operation_cancelled = false;
            },
            AppEvent::NewSessionCancel => {
                state.cancel_new_session();
            },
            AppEvent::NewSessionNextRepo => state.new_session_next_repo(),
            AppEvent::NewSessionPrevRepo => state.new_session_prev_repo(),
            AppEvent::NewSessionConfirmRepo => state.new_session_confirm_repo(),
            AppEvent::NewSessionInputChar(ch) => state.new_session_update_branch(ch),
            AppEvent::NewSessionBackspace => state.new_session_backspace(),
            AppEvent::NewSessionCreate => {
                // Mark for async processing
                state.pending_async_action = Some(AsyncAction::CreateNewSession);
            },
            AppEvent::SearchWorkspaceInputChar(ch) => {
                if let Some(ref mut session_state) = state.new_session_state {
                    session_state.filter_text.push(ch);
                    session_state.apply_filter();
                }
            },
            AppEvent::SearchWorkspaceBackspace => {
                if let Some(ref mut session_state) = state.new_session_state {
                    session_state.filter_text.pop();
                    session_state.apply_filter();
                }
            },
            AppEvent::AttachSession => {
                if let Some(session_id) = state.get_selected_session_id() {
                    state.pending_async_action = Some(AsyncAction::AttachToContainer(session_id));
                }
            },
            AppEvent::DetachSession => {
                // Clear attached session and return to session list
                state.attached_session_id = None;
                state.current_view = View::SessionList;
                state.ui_needs_refresh = true;
            },
            AppEvent::KillContainer => {
                if let Some(session_id) = state.attached_session_id {
                    state.pending_async_action = Some(AsyncAction::KillContainer(session_id));
                }
            },
            AppEvent::ReauthenticateCredentials => {
                info!("Queueing re-authentication request");
                state.pending_async_action = Some(AsyncAction::ReauthenticateCredentials);
            },
            AppEvent::DeleteSession => {
                // Show confirmation dialog
                if let Some(session) = state.selected_session() {
                    state.show_delete_confirmation(session.id);
                }
            },
            AppEvent::SwitchToLogs => {
                // TODO: Implement view switching
            },
            AppEvent::SwitchToTerminal => {
                // TODO: Implement terminal view
            },
            AppEvent::ConfirmationToggle => {
                if let Some(ref mut dialog) = state.confirmation_dialog {
                    dialog.selected_option = !dialog.selected_option;
                }
            },
            AppEvent::ConfirmationConfirm => {
                if let Some(dialog) = state.confirmation_dialog.take() {
                    if dialog.selected_option {
                        // User confirmed, execute the action
                        match dialog.confirm_action {
                            crate::app::state::ConfirmAction::DeleteSession(session_id) => {
                                state.pending_async_action = Some(AsyncAction::DeleteSession(session_id));
                            }
                        }
                    }
                    // If not confirmed, just close the dialog
                }
            },
            AppEvent::ConfirmationCancel => {
                state.confirmation_dialog = None;
            },
            AppEvent::AuthSetupNext => {
                if let Some(ref mut auth_state) = state.auth_setup_state {
                    auth_state.selected_method = match auth_state.selected_method {
                        AuthMethod::OAuth => AuthMethod::ApiKey,
                        AuthMethod::ApiKey => AuthMethod::Skip,
                        AuthMethod::Skip => AuthMethod::OAuth,
                    };
                }
            },
            AppEvent::AuthSetupPrevious => {
                if let Some(ref mut auth_state) = state.auth_setup_state {
                    auth_state.selected_method = match auth_state.selected_method {
                        AuthMethod::OAuth => AuthMethod::Skip,
                        AuthMethod::ApiKey => AuthMethod::OAuth,
                        AuthMethod::Skip => AuthMethod::ApiKey,
                    };
                }
            },
            AppEvent::AuthSetupSelect => {
                if let Some(ref auth_state) = state.auth_setup_state {
                    match auth_state.selected_method {
                        AuthMethod::OAuth => {
                            // Mark for async OAuth processing
                            state.pending_async_action = Some(AsyncAction::AuthSetupOAuth);
                        },
                        AuthMethod::ApiKey => {
                            if auth_state.api_key_input.is_empty() {
                                // Enter API key input mode
                                if let Some(ref mut auth_state) = state.auth_setup_state {
                                    auth_state.api_key_input = "sk-".to_string();
                                    auth_state.show_cursor = true;
                                }
                            } else {
                                // Save the API key
                                state.pending_async_action = Some(AsyncAction::AuthSetupApiKey);
                            }
                        },
                        AuthMethod::Skip => {
                            // Skip auth setup and go to main screen
                            state.auth_setup_state = None;
                            state.current_view = View::SessionList;
                            state.check_current_directory_status();
                            state.pending_async_action = Some(AsyncAction::RefreshWorkspaces);
                        },
                    }
                }
            },
            AppEvent::AuthSetupCancel => {
                // Same as skip - go to main screen without auth
                state.auth_setup_state = None;
                state.current_view = View::SessionList;
                state.check_current_directory_status();
                state.pending_async_action = Some(AsyncAction::RefreshWorkspaces);
            },
            AppEvent::AuthSetupInputChar(ch) => {
                if let Some(ref mut auth_state) = state.auth_setup_state {
                    auth_state.api_key_input.push(ch);
                }
            },
            AppEvent::AuthSetupBackspace => {
                if let Some(ref mut auth_state) = state.auth_setup_state {
                    if auth_state.api_key_input.is_empty() {
                        // Exit API key input mode
                        auth_state.show_cursor = false;
                    } else {
                        auth_state.api_key_input.pop();
                    }
                }
            },
            AppEvent::AuthSetupCheckStatus => {
                // Check if authentication was completed and transition if so
                if state.auth_setup_state.is_some() && !AppState::is_first_time_setup() {
                    // Authentication completed!
                    state.auth_setup_state = None;
                    state.current_view = View::SessionList;
                    state.check_current_directory_status();
                    state.pending_async_action = Some(AsyncAction::RefreshWorkspaces);
                }
            },
            AppEvent::AuthSetupRefresh => {
                // Manual refresh - check authentication status immediately
                if let Some(ref mut auth_state) = state.auth_setup_state {
                    if !AppState::is_first_time_setup() {
                        // Authentication completed!
                        state.auth_setup_state = None;
                        state.current_view = View::SessionList;
                        state.check_current_directory_status();
                        state.pending_async_action = Some(AsyncAction::RefreshWorkspaces);
                    } else {
                        // Still waiting - update message
                        auth_state.error_message = Some("Still waiting for authentication. Complete the process in the terminal window.\n\nPress 'r' to refresh or 'Esc' to cancel.".to_string());
                    }
                }
            },
            AppEvent::AuthSetupShowCommand => {
                // Show alternative authentication methods
                if let Some(ref mut auth_state) = state.auth_setup_state {
                    auth_state.error_message = Some(
                        "📋 Alternative Authentication Methods:\n\n\
                         1. If the OAuth URL didn't appear, check the container logs\n\n\
                         2. Use API Key authentication instead (press Up/Down to switch)\n\n\
                         3. Run authentication manually in a terminal:\n\
                            docker exec -it claude-box-auth /bin/bash\n\
                            claude auth login\n\n\
                         Press 'Esc' to go back.".to_string()
                    );
                }
            },
        }
    }
}