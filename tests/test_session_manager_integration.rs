// ABOUTME: Tests for SessionManager integration into AppState
// Verifies that sessions can be created through AppState using SessionManager

use claude_box::app::AppState;
use tempfile::TempDir;
use uuid::Uuid;
use std::time::SystemTime;

#[tokio::test]
async fn test_app_state_has_session_manager() {
    // BEHAVIOR: AppState should have a session_manager field that's properly initialized
    let state = AppState::new();

    // This test will fail until we add session_manager field to AppState
    // The behavior we want is to be able to access the session manager
    let sessions = state.session_manager.get_sessions();
    assert!(sessions.is_empty());
}

#[tokio::test]
async fn test_create_session_through_app_state() {
    // BEHAVIOR: AppState should be able to create sessions using SessionManager
    let mut state = AppState::new();
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path().to_string_lossy();

    // Initialize git repo in temp directory for worktree creation
    let git_init = std::process::Command::new("git")
        .args(&["init"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    assert!(git_init.status.success());

    // Create initial commit
    std::fs::write(temp_dir.path().join("README.md"), "test").unwrap();
    std::process::Command::new("git")
        .args(&["add", "."])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(&["commit", "-m", "Initial commit"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();

    // Use unique session name to avoid collisions
    let timestamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
    let session_name = format!("test_session_{}", timestamp);

    // This will fail until we implement session creation in AppState
    let result = state.create_session_via_manager(&workspace_path, "main", &session_name).await;

    assert!(result.is_ok());
    let session_id = result.unwrap();

    // Verify session was created
    let sessions = state.session_manager.get_sessions();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].name, session_name);
    assert_eq!(sessions[0].branch_name, "main");
}

#[tokio::test]
async fn test_session_manager_replaces_docker_session_creation() {
    // BEHAVIOR: AppState should use SessionManager instead of Docker-based creation
    let mut state = AppState::new();
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path().to_string_lossy();

    // Initialize git repo
    let git_init = std::process::Command::new("git")
        .args(&["init"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    assert!(git_init.status.success());

    // Create initial commit
    std::fs::write(temp_dir.path().join("README.md"), "test").unwrap();
    std::process::Command::new("git")
        .args(&["add", "."])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(&["commit", "-m", "Initial commit"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();

    // Use unique session name to avoid collisions
    let timestamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
    let session_name = format!("test_session_{}", timestamp);

    // The old Docker-based session creation should be replaced
    // This test verifies that we get proper tmux sessions, not Docker errors
    let session_id = state.create_session_via_manager(&workspace_path, "main", &session_name).await.unwrap();

    let session = state.session_manager.get_session(session_id).unwrap();
    assert!(session.tmux_session_name.starts_with("ciab_"));
    // The worktree path should be properly set and not empty
    assert!(!session.worktree_path.is_empty());
    // Verify the session has the correct name and branch
    assert_eq!(session.name, session_name);
    assert_eq!(session.branch_name, "main");
}

#[tokio::test]
async fn test_create_session_with_logs_replaces_docker() {
    // BEHAVIOR: create_session_with_logs should use SessionManager instead of Docker
    let mut state = AppState::new();
    let temp_dir = TempDir::new().unwrap();

    // Initialize git repo
    let git_init = std::process::Command::new("git")
        .args(&["init"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    assert!(git_init.status.success());

    // Create initial commit
    std::fs::write(temp_dir.path().join("README.md"), "test").unwrap();
    std::process::Command::new("git")
        .args(&["add", "."])
        .current_dir(&temp_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(&["commit", "-m", "Initial commit"])
        .current_dir(&temp_dir)
        .output()
        .unwrap();

    // Use unique session name to avoid collisions
    let timestamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
    let session_id = uuid::Uuid::new_v4();

    // This should use SessionManager instead of returning Docker error
    let result = state.create_session_with_logs(
        temp_dir.path(),
        "main",
        session_id,
        false,
        claude_box::models::SessionMode::Interactive,
        None,
    ).await;

    // Should succeed with SessionManager instead of failing with Docker error
    assert!(result.is_ok());

    // Verify session was created via SessionManager
    let sessions = state.session_manager.get_sessions();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, session_id);
    assert_eq!(sessions[0].branch_name, "main");
}