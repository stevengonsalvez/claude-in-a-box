// ABOUTME: Tests for tmux session attachment functionality
// Verifies that sessions can be created and attached to properly via PTY

use claude_box::tmux::session::TmuxSession;
use std::collections::HashMap;
use tempfile::TempDir;

#[tokio::test]
async fn test_tmux_session_attach_basic() {
    // BEHAVIOR: TmuxSession should be able to attach to a session
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

    // Create a tmux session
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let session_name = format!("test_attach_{}", timestamp);

    let mut env_vars = HashMap::new();
    env_vars.insert("CIAB_SESSION".to_string(), session_name.clone());

    let tmux_session = TmuxSession::create(
        &session_name,
        temp_dir.path().to_string_lossy().as_ref(),
        "bash",
        &env_vars,
    ).await;

    assert!(tmux_session.is_ok());
    let mut session = tmux_session.unwrap();

    // Verify session was created (name gets prefixed with "ciab_")
    assert_eq!(session.name, format!("ciab_{}", session_name));
    assert!(!session.is_attached());

    // Test attachment (should not fail even if PTY forwarding is incomplete)
    let attach_result = session.attach().await;

    // Should succeed in setting up attachment infrastructure
    assert!(attach_result.is_ok());
    assert!(session.is_attached());

    // Test immediate detachment
    let detach_result = session.detach().await;
    assert!(detach_result.is_ok());
    assert!(!session.is_attached());

    // Cleanup
    let _ = session.kill().await;
}

#[tokio::test]
async fn test_tmux_session_attach_pty_setup() {
    // BEHAVIOR: TmuxSession attach should properly set up PTY infrastructure
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

    // Create a tmux session
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let session_name = format!("test_pty_{}", timestamp);

    let mut env_vars = HashMap::new();
    env_vars.insert("CIAB_SESSION".to_string(), session_name.clone());

    let tmux_session = TmuxSession::create(
        &session_name,
        temp_dir.path().to_string_lossy().as_ref(),
        "bash",
        &env_vars,
    ).await;

    assert!(tmux_session.is_ok());
    let mut session = tmux_session.unwrap();

    // Test attachment
    let attach_result = session.attach().await;

    // Should succeed and set up proper state
    assert!(attach_result.is_ok());
    assert!(session.is_attached());

    // Verify that tasks are created (input and output tasks should exist)
    assert!(session.has_input_task());
    assert!(session.has_output_task());

    // Test detachment
    let detach_result = session.detach().await;
    assert!(detach_result.is_ok());
    assert!(!session.is_attached());

    // Tasks should be cleaned up
    assert!(!session.has_input_task());
    assert!(!session.has_output_task());

    // Cleanup
    let _ = session.kill().await;
}

#[tokio::test]
async fn test_tmux_session_detach_key_handling() {
    // BEHAVIOR: TmuxSession should detach when Ctrl+Q (ASCII 17) is received in input
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

    // Create a tmux session
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let session_name = format!("test_detach_key_{}", timestamp);

    let mut env_vars = HashMap::new();
    env_vars.insert("CIAB_SESSION".to_string(), session_name.clone());

    let tmux_session = TmuxSession::create(
        &session_name,
        temp_dir.path().to_string_lossy().as_ref(),
        "bash",
        &env_vars,
    ).await;

    assert!(tmux_session.is_ok());
    let mut session = tmux_session.unwrap();

    // Attach to session - this should return a detach receiver
    let attach_result = session.attach().await;
    assert!(attach_result.is_ok());
    assert!(session.is_attached());

    // The receiver should be available for listening to detach signals
    let detach_receiver = attach_result.unwrap();

    // Verify that input and output tasks are running
    assert!(session.has_input_task());
    assert!(session.has_output_task());

    // TODO: In a full implementation, we would simulate Ctrl+Q input
    // For now, test that we can manually detach and it cleans up properly
    let manual_detach = session.detach().await;
    assert!(manual_detach.is_ok());
    assert!(!session.is_attached());

    // Verify tasks are cleaned up after detach
    assert!(!session.has_input_task());
    assert!(!session.has_output_task());

    // Cleanup
    let _ = session.kill().await;
}