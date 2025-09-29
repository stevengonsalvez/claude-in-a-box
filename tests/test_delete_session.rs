// ABOUTME: Tests for session deletion functionality to ensure non-blocking UI operations
// and proper state cleanup when deleting sessions

use std::time::Duration;
use tokio::time::Instant;
use uuid::Uuid;

use claude_box::app::AppState;
use claude_box::models::{Session, SessionStatus, Workspace};
use claude_box::app::state::AsyncAction;

// Factory function to create a test session
fn create_test_session(overrides: Option<&dyn Fn(&mut Session)>) -> Session {
    let mut session = Session::new(
        "test-session".to_string(),
        "/tmp/test".to_string(),
    );
    session.set_status(SessionStatus::Running);

    if let Some(override_fn) = overrides {
        override_fn(&mut session);
    }

    session
}

// Factory function to create a test workspace with sessions
fn create_test_workspace_with_sessions(sessions: Vec<Session>) -> Workspace {
    let mut workspace = Workspace::new(
        "test-workspace".to_string(),
        std::path::PathBuf::from("/tmp/test-workspace"),
    );

    for session in sessions {
        workspace.add_session(session);
    }

    workspace
}

#[tokio::test]
async fn test_delete_session_should_be_fast_and_non_blocking() {
    // Arrange
    let mut app_state = AppState::new();

    // Create a test session
    let session = create_test_session(None);
    let session_id = session.id;

    // Add session to app state
    let workspace = create_test_workspace_with_sessions(vec![session]);
    app_state.workspaces = vec![workspace];

    // Verify session exists before deletion
    let session_count_before = app_state.workspaces.iter()
        .map(|w| w.sessions.len())
        .sum::<usize>();
    assert_eq!(session_count_before, 1);

    // Act - measure deletion time
    let start = Instant::now();

    // Trigger deletion via async action
    app_state.pending_async_action = Some(AsyncAction::DeleteSession(session_id));
    app_state.process_async_action().await.expect("Failed to process delete action");

    let duration = start.elapsed();

    // Assert - deletion should be fast (under 100ms for UI responsiveness)
    assert!(duration < Duration::from_millis(100),
        "Delete operation took {}ms, should be under 100ms for UI responsiveness",
        duration.as_millis());

    // Verify session was removed from UI state
    let session_count_after = app_state.workspaces.iter()
        .map(|w| w.sessions.len())
        .sum::<usize>();
    assert_eq!(session_count_after, 0);

    // Verify UI refresh flag is set
    assert!(app_state.ui_needs_refresh, "UI should be marked for refresh after deletion");
}

#[tokio::test]
async fn test_delete_session_removes_session_from_workspace() {
    // Arrange
    let mut app_state = AppState::new();

    let session1 = create_test_session(Some(&|s| s.name = "session1".to_string()));
    let session2 = create_test_session(Some(&|s| s.name = "session2".to_string()));
    let session_to_delete_id = session1.id;

    let workspace = create_test_workspace_with_sessions(vec![session1, session2]);
    app_state.workspaces = vec![workspace];

    // Act
    app_state.pending_async_action = Some(AsyncAction::DeleteSession(session_to_delete_id));
    app_state.process_async_action().await.expect("Failed to process delete action");

    // Assert - only one session should remain
    let remaining_sessions: Vec<&Session> = app_state.workspaces.iter()
        .flat_map(|w| &w.sessions)
        .collect();

    assert_eq!(remaining_sessions.len(), 1);
    assert_eq!(remaining_sessions[0].name, "session2");
    assert_ne!(remaining_sessions[0].id, session_to_delete_id);
}

#[tokio::test]
async fn test_delete_session_handles_nonexistent_session_gracefully() {
    // Arrange
    let mut app_state = AppState::new();
    let nonexistent_session_id = Uuid::new_v4();

    // Add some sessions to verify they aren't affected
    let session = create_test_session(None);
    let workspace = create_test_workspace_with_sessions(vec![session]);
    app_state.workspaces = vec![workspace];

    let session_count_before = app_state.workspaces.iter()
        .map(|w| w.sessions.len())
        .sum::<usize>();

    // Act - try to delete nonexistent session
    app_state.pending_async_action = Some(AsyncAction::DeleteSession(nonexistent_session_id));
    let result = app_state.process_async_action().await;

    // Assert - should not fail and should not affect existing sessions
    assert!(result.is_ok(), "Deleting nonexistent session should not fail");

    let session_count_after = app_state.workspaces.iter()
        .map(|w| w.sessions.len())
        .sum::<usize>();
    assert_eq!(session_count_before, session_count_after, "Existing sessions should not be affected");
}