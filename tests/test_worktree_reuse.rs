// ABOUTME: Tests for worktree reuse functionality - finding and reusing existing worktrees for new sessions

use claude_box::docker::{SessionLifecycleManager, session_lifecycle::SessionRequest};
use claude_box::git::WorktreeManager;
use claude_box::models::SessionMode;
use tempfile::TempDir;
use uuid::Uuid;

/// Test finding existing worktrees for a workspace
#[tokio::test]
async fn test_find_existing_worktrees_for_workspace() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    // Create a test git repository
    std::fs::create_dir_all(&repo_path).expect("Failed to create repo dir");
    let repo = git2::Repository::init(&repo_path).expect("Failed to init git repo");

    // Create initial commit
    let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
    let tree_id = {
        let mut index = repo.index().unwrap();
        index.write_tree().unwrap()
    };
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Initial commit",
        &tree,
        &[],
    )
    .unwrap();

    let mut worktree_manager = WorktreeManager::new().expect("Failed to create worktree manager");

    // Create first worktree
    let session_id_1 = Uuid::new_v4();
    let worktree_1 = worktree_manager
        .create_worktree(session_id_1, &repo_path, "claude/feature-1", None)
        .expect("Failed to create first worktree");

    // Create second worktree
    let session_id_2 = Uuid::new_v4();
    let worktree_2 = worktree_manager
        .create_worktree(session_id_2, &repo_path, "claude/feature-2", None)
        .expect("Failed to create second worktree");

    // Test finding worktrees for this workspace
    let found_worktrees = worktree_manager
        .find_worktrees_for_workspace(&repo_path)
        .expect("Failed to find worktrees");

    assert_eq!(found_worktrees.len(), 2, "Should find exactly 2 worktrees");

    // Verify the found worktrees match what we created
    let found_ids: Vec<Uuid> = found_worktrees.iter().map(|w| w.id).collect();
    assert!(
        found_ids.contains(&session_id_1),
        "Should find first worktree"
    );
    assert!(
        found_ids.contains(&session_id_2),
        "Should find second worktree"
    );

    // Cleanup
    worktree_manager
        .remove_worktree(session_id_1)
        .expect("Failed to remove first worktree");
    worktree_manager
        .remove_worktree(session_id_2)
        .expect("Failed to remove second worktree");
}

/// Test finding worktrees when none exist
#[tokio::test]
async fn test_find_worktrees_empty_result() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path().join("empty_repo");

    // Create a test git repository with no worktrees
    std::fs::create_dir_all(&repo_path).expect("Failed to create repo dir");
    git2::Repository::init(&repo_path).expect("Failed to init git repo");

    let worktree_manager = WorktreeManager::new().expect("Failed to create worktree manager");

    let found_worktrees = worktree_manager
        .find_worktrees_for_workspace(&repo_path)
        .expect("Failed to find worktrees");

    assert_eq!(
        found_worktrees.len(),
        0,
        "Should find no worktrees for empty repo"
    );
}

/// Test creating a session with an existing worktree
#[tokio::test]
async fn test_create_session_with_existing_worktree() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    // Create a test git repository
    std::fs::create_dir_all(&repo_path).expect("Failed to create repo dir");
    let repo = git2::Repository::init(&repo_path).expect("Failed to init git repo");

    // Create initial commit
    let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
    let tree_id = {
        let mut index = repo.index().unwrap();
        index.write_tree().unwrap()
    };
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Initial commit",
        &tree,
        &[],
    )
    .unwrap();

    let mut worktree_manager = WorktreeManager::new().expect("Failed to create worktree manager");

    // Create initial worktree (simulating a previous session)
    let original_session_id = Uuid::new_v4();
    let existing_worktree = worktree_manager
        .create_worktree(
            original_session_id,
            &repo_path,
            "claude/feature-branch",
            None,
        )
        .expect("Failed to create initial worktree");

    // Now test creating a new session with the existing worktree
    let mut lifecycle_manager = SessionLifecycleManager::new()
        .await
        .expect("Failed to create lifecycle manager");

    let new_session_id = Uuid::new_v4();
    let request = SessionRequest {
        session_id: new_session_id,
        workspace_name: "test_workspace".to_string(),
        workspace_path: repo_path.clone(),
        branch_name: "claude/feature-branch".to_string(),
        base_branch: None,
        container_config: None,
        skip_permissions: false,
        mode: SessionMode::Interactive,
        boss_prompt: None,
    };

    // This should reuse the existing worktree instead of creating a new one
    let session_state = lifecycle_manager
        .create_session_with_existing_worktree(request, existing_worktree.clone())
        .await
        .expect("Failed to create session with existing worktree");

    // Verify the session was created correctly
    assert_eq!(session_state.session.id, new_session_id);
    assert_eq!(session_state.session.branch_name, "claude/feature-branch");
    assert!(session_state.worktree_info.is_some());

    // Verify it's using the same worktree path
    let worktree_info = session_state.worktree_info.unwrap();
    assert_eq!(worktree_info.path, existing_worktree.path);
    assert_eq!(worktree_info.branch_name, existing_worktree.branch_name);

    // Cleanup
    lifecycle_manager
        .remove_session(new_session_id)
        .await
        .expect("Failed to remove session");
    worktree_manager
        .remove_worktree(original_session_id)
        .expect("Failed to remove worktree");
}

/// Test that reusing worktree preserves git state
#[tokio::test]
async fn test_worktree_reuse_preserves_git_state() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    // Create a test git repository
    std::fs::create_dir_all(&repo_path).expect("Failed to create repo dir");
    let repo = git2::Repository::init(&repo_path).expect("Failed to init git repo");

    // Create initial commit
    let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
    let tree_id = {
        let mut index = repo.index().unwrap();
        index.write_tree().unwrap()
    };
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Initial commit",
        &tree,
        &[],
    )
    .unwrap();

    let mut worktree_manager = WorktreeManager::new().expect("Failed to create worktree manager");

    // Create worktree and add some changes
    let original_session_id = Uuid::new_v4();
    let existing_worktree = worktree_manager
        .create_worktree(
            original_session_id,
            &repo_path,
            "claude/feature-branch",
            None,
        )
        .expect("Failed to create initial worktree");

    // Add a test file to the worktree
    let test_file_path = existing_worktree.path.join("test_file.txt");
    std::fs::write(&test_file_path, "test content").expect("Failed to write test file");

    // Verify file exists before reuse
    assert!(
        test_file_path.exists(),
        "Test file should exist in worktree"
    );

    // Create new session with existing worktree
    let mut lifecycle_manager = SessionLifecycleManager::new()
        .await
        .expect("Failed to create lifecycle manager");

    let new_session_id = Uuid::new_v4();
    let request = SessionRequest {
        session_id: new_session_id,
        workspace_name: "test_workspace".to_string(),
        workspace_path: repo_path.clone(),
        branch_name: "claude/feature-branch".to_string(),
        base_branch: None,
        container_config: None,
        skip_permissions: false,
        mode: SessionMode::Boss,
        boss_prompt: Some("test prompt".to_string()),
    };

    let session_state = lifecycle_manager
        .create_session_with_existing_worktree(request, existing_worktree.clone())
        .await
        .expect("Failed to create session with existing worktree");

    // Verify the file still exists after reuse
    assert!(
        test_file_path.exists(),
        "Test file should still exist after worktree reuse"
    );

    // Verify git state is preserved
    let worktree_info = session_state.worktree_info.unwrap();
    assert_eq!(worktree_info.branch_name, "claude/feature-branch");

    // Cleanup
    lifecycle_manager
        .remove_session(new_session_id)
        .await
        .expect("Failed to remove session");
    worktree_manager
        .remove_worktree(original_session_id)
        .expect("Failed to remove worktree");
}
