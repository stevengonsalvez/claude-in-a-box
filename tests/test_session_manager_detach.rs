// ABOUTME: Tests for SessionManager detach functionality integration
// Verifies that detaching from sessions updates status correctly

use claude_box::session::SessionManager;
use claude_box::models::SessionStatus;
use tempfile::TempDir;
use std::time::SystemTime;

#[tokio::test]
async fn test_session_manager_detach_updates_status() {
    // BEHAVIOR: SessionManager should update session status to Detached when detaching
    let temp_dir = TempDir::new().unwrap();

    // Initialize git repo for the session
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

    let mut session_manager = SessionManager::new();

    // Create a session
    let timestamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
    let session_name = format!("test_detach_status_{}", timestamp);

    let session_id = session_manager.create_session(
        temp_dir.path().to_string_lossy().as_ref(),
        "main",
        &session_name,
    ).await;

    assert!(session_id.is_ok());
    let session_id = session_id.unwrap();

    // Verify initial status is Running
    let session = session_manager.get_session(session_id).unwrap();
    assert_eq!(session.status, SessionStatus::Running);

    // Attach to the session
    let attach_result = session_manager.attach_session(session_id).await;
    assert!(attach_result.is_ok());

    // Verify status changed to Attached
    let session = session_manager.get_session(session_id).unwrap();
    assert_eq!(session.status, SessionStatus::Attached);

    // Detach from the session
    let detach_result = session_manager.detach_session(session_id).await;
    assert!(detach_result.is_ok());

    // Verify status changed to Detached
    let session = session_manager.get_session(session_id).unwrap();
    assert_eq!(session.status, SessionStatus::Detached);

    // Cleanup
    let _ = session_manager.cleanup_session(session_id).await;
}

#[tokio::test]
async fn test_session_manager_detach_preserves_tmux_session() {
    // BEHAVIOR: SessionManager detach should preserve the tmux session for later reattachment
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

    let mut session_manager = SessionManager::new();

    // Create a session
    let timestamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
    let session_name = format!("test_preserve_{}", timestamp);

    let session_id = session_manager.create_session(
        temp_dir.path().to_string_lossy().as_ref(),
        "main",
        &session_name,
    ).await;

    assert!(session_id.is_ok());
    let session_id = session_id.unwrap();

    // Get the tmux session name
    let session = session_manager.get_session(session_id).unwrap();
    let tmux_session_name = session.tmux_session_name.clone();

    // Attach and then detach
    let _ = session_manager.attach_session(session_id).await;
    let detach_result = session_manager.detach_session(session_id).await;
    assert!(detach_result.is_ok());

    // Verify the tmux session still exists by checking tmux list-sessions
    let tmux_list = tokio::process::Command::new("tmux")
        .args(&["list-sessions", "-F", "#{session_name}"])
        .output()
        .await
        .unwrap();

    assert!(tmux_list.status.success());
    let session_list = String::from_utf8_lossy(&tmux_list.stdout);
    assert!(session_list.contains(&tmux_session_name),
        "Tmux session '{}' should still exist after detach. Available sessions: {}",
        tmux_session_name, session_list);

    // Should be able to re-attach to the preserved session
    let reattach_result = session_manager.attach_session(session_id).await;
    assert!(reattach_result.is_ok());

    // Verify status is back to Attached
    let session = session_manager.get_session(session_id).unwrap();
    assert_eq!(session.status, SessionStatus::Attached);

    // Cleanup
    let _ = session_manager.cleanup_session(session_id).await;
}