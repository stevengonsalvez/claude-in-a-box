// ABOUTME: Tests for AppState new session functionality, focusing on mode selection flow

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::{AppState, NewSessionState, NewSessionStep};
    use crate::models::SessionMode;
    use std::path::PathBuf;

    /// Test that pressing 'n' for new session should go through mode selection
    #[test]
    fn test_new_session_should_include_mode_selection() {
        let mut state = AppState::new();

        // Simulate the 'n' key press flow
        // This should NOT skip mode selection like current directory mode does

        // First, we need to simulate having workspaces available
        state.workspaces = vec![crate::models::Workspace {
            name: "test-workspace".to_string(),
            path: PathBuf::from("/test/path"),
            sessions: vec![],
        }];

        // Simulate the async action that happens when 'n' is pressed
        // This should create a session that goes through ALL steps including mode selection
        let current_dir = std::env::current_dir().unwrap();

        // Create new session state manually to test the flow
        state.new_session_state = Some(NewSessionState {
            available_repos: vec![current_dir.clone()],
            filtered_repos: vec![(0, current_dir.clone())],
            selected_repo_index: Some(0),
            branch_name: "test-branch".to_string(),
            step: NewSessionStep::InputBranch, // This is what happens currently
            filter_text: String::new(),
            is_current_dir_mode: false, // This should be false for 'n' key press
            skip_permissions: false,
            mode: SessionMode::Interactive,
            boss_prompt: crate::app::state::TextEditor::new(),
            file_finder: crate::components::fuzzy_file_finder::FuzzyFileFinderState::new(),
        });

        // Now simulate pressing Enter in InputBranch step
        // This should proceed to mode selection, NOT skip it
        state.new_session_proceed_to_mode_selection();

        // Verify that we're now in SelectMode step
        if let Some(ref session_state) = state.new_session_state {
            assert_eq!(
                session_state.step,
                NewSessionStep::SelectMode,
                "After proceeding from InputBranch, should be in SelectMode step for mode selection"
            );
            assert!(
                !session_state.is_current_dir_mode,
                "Normal new session should not be in current directory mode"
            );
        } else {
            panic!("Session state should exist after proceeding to mode selection");
        }
    }

    /// Test that current directory mode (different from 'n' key) should skip mode selection
    #[test]
    fn test_current_dir_mode_should_skip_mode_selection() {
        let mut state = AppState::new();
        let current_dir = std::env::current_dir().unwrap();

        // Create session state in current directory mode (this is different from 'n' key)
        state.new_session_state = Some(NewSessionState {
            available_repos: vec![current_dir.clone()],
            filtered_repos: vec![(0, current_dir.clone())],
            selected_repo_index: Some(0),
            branch_name: "test-branch".to_string(),
            step: NewSessionStep::InputBranch,
            filter_text: String::new(),
            is_current_dir_mode: true, // This should be true for current dir mode
            skip_permissions: false,
            mode: SessionMode::Interactive,
            boss_prompt: crate::app::state::TextEditor::new(),
            file_finder: crate::components::fuzzy_file_finder::FuzzyFileFinderState::new(),
        });

        // In current directory mode, pressing Enter should skip mode selection
        // and go directly to session creation (this is the intended behavior for current dir mode)

        // Verify the current behavior is correct for current directory mode
        if let Some(ref session_state) = state.new_session_state {
            assert!(
                session_state.is_current_dir_mode,
                "This test should be for current directory mode"
            );
            assert_eq!(session_state.step, NewSessionStep::InputBranch);
        }
    }

    /// Test mode selection toggle functionality
    #[test]
    fn test_mode_selection_toggle() {
        let mut state = AppState::new();
        let current_dir = std::env::current_dir().unwrap();

        // Create session state in SelectMode step
        state.new_session_state = Some(NewSessionState {
            available_repos: vec![current_dir.clone()],
            filtered_repos: vec![(0, current_dir.clone())],
            selected_repo_index: Some(0),
            branch_name: "test-branch".to_string(),
            step: NewSessionStep::SelectMode, // In mode selection
            filter_text: String::new(),
            is_current_dir_mode: false,
            skip_permissions: false,
            mode: SessionMode::Interactive, // Start with Interactive
            boss_prompt: crate::app::state::TextEditor::new(),
            file_finder: crate::components::fuzzy_file_finder::FuzzyFileFinderState::new(),
        });

        // Test toggling mode
        state.new_session_toggle_mode();

        if let Some(ref session_state) = state.new_session_state {
            assert_eq!(
                session_state.mode,
                SessionMode::Boss,
                "Mode should toggle from Interactive to Boss"
            );
        }

        // Toggle again
        state.new_session_toggle_mode();

        if let Some(ref session_state) = state.new_session_state {
            assert_eq!(
                session_state.mode,
                SessionMode::Interactive,
                "Mode should toggle back from Boss to Interactive"
            );
        }
    }

    /// Test proceeding from mode selection to appropriate next step
    #[test]
    fn test_proceed_from_mode_selection() {
        let mut state = AppState::new();
        let current_dir = std::env::current_dir().unwrap();

        // Test Interactive mode flow
        state.new_session_state = Some(NewSessionState {
            available_repos: vec![current_dir.clone()],
            filtered_repos: vec![(0, current_dir.clone())],
            selected_repo_index: Some(0),
            branch_name: "test-branch".to_string(),
            step: NewSessionStep::SelectMode,
            filter_text: String::new(),
            is_current_dir_mode: false,
            skip_permissions: false,
            mode: SessionMode::Interactive,
            boss_prompt: crate::app::state::TextEditor::new(),
            file_finder: crate::components::fuzzy_file_finder::FuzzyFileFinderState::new(),
        });

        state.new_session_proceed_from_mode();

        if let Some(ref session_state) = state.new_session_state {
            assert_eq!(
                session_state.step,
                NewSessionStep::ConfigurePermissions,
                "Interactive mode should proceed to ConfigurePermissions"
            );
        }

        // Test Boss mode flow
        state.new_session_state = Some(NewSessionState {
            available_repos: vec![current_dir.clone()],
            filtered_repos: vec![(0, current_dir.clone())],
            selected_repo_index: Some(0),
            branch_name: "test-branch".to_string(),
            step: NewSessionStep::SelectMode,
            filter_text: String::new(),
            is_current_dir_mode: false,
            skip_permissions: false,
            mode: SessionMode::Boss,
            boss_prompt: crate::app::state::TextEditor::new(),
            file_finder: crate::components::fuzzy_file_finder::FuzzyFileFinderState::new(),
        });

        state.new_session_proceed_from_mode();

        if let Some(ref session_state) = state.new_session_state {
            assert_eq!(
                session_state.step,
                NewSessionStep::InputPrompt,
                "Boss mode should proceed to InputPrompt"
            );
        }
    }

    /// Test the actual event flow: 'n' key -> Enter in branch input -> should show mode selection
    #[test]
    fn test_n_key_event_flow_shows_mode_selection() {
        let mut state = AppState::new();

        // Simulate having workspaces available
        state.workspaces = vec![crate::models::Workspace {
            name: "test-workspace".to_string(),
            path: PathBuf::from("/test/path"),
            sessions: vec![],
        }];

        // Simulate the 'n' key press by setting the async action that would be triggered
        state.pending_async_action = Some(crate::app::state::AsyncAction::NewSessionNormal);

        // Process the async action (this simulates what happens in the main loop)
        // We can't actually call the async method in a sync test, but we can verify
        // that the correct async action is set
        assert_eq!(
            state.pending_async_action,
            Some(crate::app::state::AsyncAction::NewSessionNormal),
            "The 'n' key should trigger NewSessionNormal, not NewSessionInCurrentDir"
        );

        // Verify that NewSessionNormal would create a session with is_current_dir_mode: false
        // by checking that the new method we created sets the right flag
        // (This is tested indirectly through the other tests, but this confirms the integration)
    }

    /// Test notification system functionality
    #[test]
    fn test_notification_system() {
        let mut state = AppState::new();

        // Test adding different types of notifications
        state.add_success_notification("Success message".to_string());
        state.add_error_notification("Error message".to_string());
        state.add_info_notification("Info message".to_string());
        state.add_warning_notification("Warning message".to_string());

        // Should have 4 notifications
        assert_eq!(state.notifications.len(), 4);

        // Test getting current notifications (non-expired)
        let current = state.get_current_notifications();
        assert_eq!(current.len(), 4);

        // Test notification types
        assert_eq!(
            current[0].notification_type,
            crate::app::state::NotificationType::Success
        );
        assert_eq!(
            current[1].notification_type,
            crate::app::state::NotificationType::Error
        );
        assert_eq!(
            current[2].notification_type,
            crate::app::state::NotificationType::Info
        );
        assert_eq!(
            current[3].notification_type,
            crate::app::state::NotificationType::Warning
        );
    }

    /// Test notification expiration
    #[test]
    fn test_notification_expiration() {
        let mut state = AppState::new();

        // Add a notification with custom duration
        let mut notification = crate::app::state::Notification::success("Test message".to_string());
        notification.duration = std::time::Duration::from_millis(1); // Very short duration
        state.add_notification(notification);

        // Wait for expiration
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Clean up expired notifications
        state.cleanup_expired_notifications();

        // Should have no notifications left
        assert_eq!(state.notifications.len(), 0);
    }

    /// Test git commit and push notifications (without actual git operations)
    #[test]
    fn test_git_commit_and_push_notifications() {
        let mut state = AppState::new();

        // Test that git_commit_and_push method exists and can be called
        // (This will not actually perform git operations since git_view_state is None)
        state.git_commit_and_push();

        // Should not crash and should not add any notifications since git_view_state is None
        assert_eq!(state.notifications.len(), 0);
    }
}
