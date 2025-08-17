// ABOUTME: Tests for session restart functionality - recreating stopped sessions with their existing worktrees

use claude_box::docker::{SessionLifecycleManager, session_lifecycle::SessionRequest};
use claude_box::models::{SessionMode, SessionStatus};
use std::path::Path;
use tempfile::TempDir;
use uuid::Uuid;

/// Helper function to create a proper test git repository
fn create_test_git_repo(repo_path: &Path) -> git2::Repository {
    std::fs::create_dir_all(repo_path).expect("Failed to create repo dir");
    let repo = git2::Repository::init(repo_path).expect("Failed to init git repo");

    // Set up git configuration
    let mut config = repo.config().expect("Failed to get git config");
    config.set_str("user.name", "Test User").expect("Failed to set user.name");
    config
        .set_str("user.email", "test@example.com")
        .expect("Failed to set user.email");

    // Create a real file for the initial commit
    let readme_path = repo_path.join("README.md");
    std::fs::write(
        &readme_path,
        "# Test Repository\n\nThis is a test repository for session restart tests.\n",
    )
    .expect("Failed to write README.md");

    // Add the file to git index
    let mut index = repo.index().expect("Failed to get git index");
    index
        .add_path(Path::new("README.md"))
        .expect("Failed to add README.md to index");
    index.write().expect("Failed to write index");

    // Create initial commit with the file
    let signature =
        git2::Signature::now("Test User", "test@example.com").expect("Failed to create signature");
    let tree_id = index.write_tree().expect("Failed to write tree");
    let tree = repo.find_tree(tree_id).expect("Failed to find tree");

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Initial commit",
        &tree,
        &[],
    )
    .expect("Failed to create initial commit");

    // Drop the tree explicitly to release the borrow
    drop(tree);

    repo
}

/// Test recreating a stopped session with its existing worktree
#[tokio::test]
async fn test_recreate_stopped_session() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    // Create a proper test git repository
    let _repo = create_test_git_repo(&repo_path);

    let mut lifecycle_manager = SessionLifecycleManager::new()
        .await
        .expect("Failed to create lifecycle manager");

    let session_id = Uuid::new_v4();
    let request = SessionRequest {
        session_id,
        workspace_name: "test_workspace".to_string(),
        workspace_path: repo_path.clone(),
        branch_name: "claude/feature-branch".to_string(),
        base_branch: None,
        container_config: None,
        skip_permissions: false,
        mode: SessionMode::Interactive,
        boss_prompt: None,
    };

    // Create initial session
    let session_state = lifecycle_manager
        .create_session(request, None)
        .await
        .expect("Failed to create session");

    println!("Initial session status: {:?}", session_state.session.status);
    println!(
        "Initial session has container: {}",
        session_state.container.is_some()
    );

    // The session should be running after creation
    assert_eq!(session_state.session.status, SessionStatus::Running);
    assert!(session_state.worktree_info.is_some());
    assert!(session_state.container.is_some());

    let original_worktree_path = session_state.worktree_info.as_ref().unwrap().path.clone();

    // Add a test file to the worktree to verify state preservation
    let test_file_path = original_worktree_path.join("test_file.txt");
    std::fs::write(&test_file_path, "test content").expect("Failed to write test file");

    // Stop the session
    lifecycle_manager
        .stop_session(session_id)
        .await
        .expect("Failed to stop session");

    // Verify session is stopped
    let stopped_session = lifecycle_manager.get_session(session_id).unwrap();
    println!(
        "After stop - session status: {:?}",
        stopped_session.session.status
    );
    assert_eq!(stopped_session.session.status, SessionStatus::Stopped);

    // Recreate the session
    lifecycle_manager
        .recreate_session(session_id)
        .await
        .expect("Failed to recreate session");

    // Verify session is running again
    let recreated_session = lifecycle_manager.get_session(session_id).unwrap();
    println!(
        "After recreate - session status: {:?}",
        recreated_session.session.status
    );
    assert_eq!(recreated_session.session.status, SessionStatus::Running);
    assert!(recreated_session.container.is_some());

    // Verify worktree is the same and file still exists
    let recreated_worktree_path = recreated_session.worktree_info.as_ref().unwrap().path.clone();
    assert_eq!(original_worktree_path, recreated_worktree_path);
    assert!(
        test_file_path.exists(),
        "Test file should still exist after recreation"
    );

    // Cleanup
    lifecycle_manager
        .remove_session(session_id)
        .await
        .expect("Failed to remove session");
}

/// Test that recreating a running session fails
#[tokio::test]
async fn test_recreate_running_session_fails() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    // Create a proper test git repository
    let _repo = create_test_git_repo(&repo_path);

    let mut lifecycle_manager = SessionLifecycleManager::new()
        .await
        .expect("Failed to create lifecycle manager");

    let session_id = Uuid::new_v4();
    let request = SessionRequest {
        session_id,
        workspace_name: "test_workspace".to_string(),
        workspace_path: repo_path.clone(),
        branch_name: "claude/feature-branch".to_string(),
        base_branch: None,
        container_config: None,
        skip_permissions: false,
        mode: SessionMode::Interactive,
        boss_prompt: None,
    };

    // Create session (it will be running)
    let session_state = lifecycle_manager
        .create_session(request, None)
        .await
        .expect("Failed to create session");

    println!(
        "Session status before recreate attempt: {:?}",
        session_state.session.status
    );

    // Try to recreate running session - should fail
    let result = lifecycle_manager.recreate_session(session_id).await;
    println!("Recreate result: {:?}", result);
    assert!(result.is_err(), "Recreating a running session should fail");

    // Cleanup
    lifecycle_manager
        .remove_session(session_id)
        .await
        .expect("Failed to remove session");
}

/// Test that recreating a non-existent session fails
#[tokio::test]
async fn test_recreate_nonexistent_session_fails() {
    let lifecycle_manager = SessionLifecycleManager::new()
        .await
        .expect("Failed to create lifecycle manager");

    let non_existent_session_id = Uuid::new_v4();

    // Try to recreate non-existent session - should fail
    let mut lifecycle_manager = lifecycle_manager;
    let result = lifecycle_manager.recreate_session(non_existent_session_id).await;
    assert!(
        result.is_err(),
        "Recreating a non-existent session should fail"
    );
}

/// Test recreating a session preserves git state and changes
#[tokio::test]
async fn test_recreate_session_preserves_git_state() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    // Create a proper test git repository
    let _repo = create_test_git_repo(&repo_path);

    let mut lifecycle_manager = SessionLifecycleManager::new()
        .await
        .expect("Failed to create lifecycle manager");

    let session_id = Uuid::new_v4();
    let request = SessionRequest {
        session_id,
        workspace_name: "test_workspace".to_string(),
        workspace_path: repo_path.clone(),
        branch_name: "claude/feature-branch".to_string(),
        base_branch: None,
        container_config: None,
        skip_permissions: false,
        mode: SessionMode::Boss,
        boss_prompt: Some("test prompt".to_string()),
    };

    // Create session
    let session_state = lifecycle_manager
        .create_session(request, None)
        .await
        .expect("Failed to create session");

    let worktree_path = session_state.worktree_info.as_ref().unwrap().path.clone();

    // Add multiple files to simulate work done in the session
    let file1_path = worktree_path.join("feature.rs");
    let file2_path = worktree_path.join("tests.rs");
    let file3_path = worktree_path.join("README.md");

    std::fs::write(&file1_path, "// Feature implementation\nfn main() {}")
        .expect("Failed to write feature file");
    std::fs::write(&file2_path, "// Test cases\n#[test]\nfn test_feature() {}")
        .expect("Failed to write test file");
    std::fs::write(&file3_path, "# Feature Branch\n\nThis is a new feature.")
        .expect("Failed to write README");

    // Stop the session
    lifecycle_manager
        .stop_session(session_id)
        .await
        .expect("Failed to stop session");

    // Recreate the session
    lifecycle_manager
        .recreate_session(session_id)
        .await
        .expect("Failed to recreate session");

    // Verify all files still exist and have correct content
    assert!(file1_path.exists(), "Feature file should still exist");
    assert!(file2_path.exists(), "Test file should still exist");
    assert!(file3_path.exists(), "README file should still exist");

    let feature_content =
        std::fs::read_to_string(&file1_path).expect("Failed to read feature file");
    assert!(
        feature_content.contains("Feature implementation"),
        "Feature file content should be preserved"
    );

    let test_content = std::fs::read_to_string(&file2_path).expect("Failed to read test file");
    assert!(
        test_content.contains("Test cases"),
        "Test file content should be preserved"
    );

    let readme_content = std::fs::read_to_string(&file3_path).expect("Failed to read README");
    assert!(
        readme_content.contains("Feature Branch"),
        "README content should be preserved"
    );

    // Verify git branch is still correct
    let recreated_session = lifecycle_manager.get_session(session_id).unwrap();
    assert_eq!(
        recreated_session.session.branch_name,
        "claude/feature-branch"
    );

    // Cleanup
    lifecycle_manager
        .remove_session(session_id)
        .await
        .expect("Failed to remove session");
}
