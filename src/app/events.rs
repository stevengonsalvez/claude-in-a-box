// ABOUTME: Event handling system for keyboard input and app actions

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::app::{AppState, state::AsyncAction};

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
    StartStopSession,
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
            KeyCode::Char('r') => Some(AppEvent::StartStopSession),  // Changed from 's' to 'r' (run/stop)
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
                // TODO: Implement session attachment
            },
            AppEvent::StartStopSession => {
                // TODO: Implement start/stop session
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
        }
    }
}