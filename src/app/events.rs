// ABOUTME: Event handling system for keyboard input and app actions

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::app::AppState;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Quit,
    NextSession,
    PreviousSession,
    NextWorkspace,
    PreviousWorkspace,
    ToggleHelp,
    NewSession,
    AttachSession,
    StartStopSession,
    DeleteSession,
    SwitchToLogs,
    SwitchToTerminal,
    GoToTop,
    GoToBottom,
}

pub struct EventHandler;

impl EventHandler {
    pub fn handle_key_event(key_event: KeyEvent, state: &mut AppState) -> Option<AppEvent> {
        if state.help_visible {
            match key_event.code {
                KeyCode::Char('?') | KeyCode::Esc => return Some(AppEvent::ToggleHelp),
                _ => return None,
            }
        }

        match key_event.code {
            KeyCode::Char('q') | KeyCode::Esc => Some(AppEvent::Quit),
            KeyCode::Char('?') => Some(AppEvent::ToggleHelp),
            KeyCode::Char('j') | KeyCode::Down => Some(AppEvent::NextSession),
            KeyCode::Char('k') | KeyCode::Up => Some(AppEvent::PreviousSession),
            KeyCode::Char('h') | KeyCode::Left => Some(AppEvent::PreviousWorkspace),
            KeyCode::Char('l') | KeyCode::Right => Some(AppEvent::NextWorkspace),
            KeyCode::Char('g') => Some(AppEvent::GoToTop),
            KeyCode::Char('G') => Some(AppEvent::GoToBottom),
            KeyCode::Char('n') => Some(AppEvent::NewSession),
            KeyCode::Char('a') => Some(AppEvent::AttachSession),
            KeyCode::Char('s') => Some(AppEvent::StartStopSession),
            KeyCode::Char('d') => Some(AppEvent::DeleteSession),
            KeyCode::Tab => Some(AppEvent::SwitchToLogs),
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => Some(AppEvent::Quit),
            _ => None,
        }
    }

    pub fn process_event(event: AppEvent, state: &mut AppState) {
        match event {
            AppEvent::Quit => state.quit(),
            AppEvent::ToggleHelp => state.toggle_help(),
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
                // TODO: Implement new session creation
            },
            AppEvent::AttachSession => {
                // TODO: Implement session attachment
            },
            AppEvent::StartStopSession => {
                // TODO: Implement start/stop session
            },
            AppEvent::DeleteSession => {
                // TODO: Implement session deletion
            },
            AppEvent::SwitchToLogs => {
                // TODO: Implement view switching
            },
            AppEvent::SwitchToTerminal => {
                // TODO: Implement terminal view
            },
        }
    }
}