// ABOUTME: Git worktree management for creating isolated working directories for sessions

use anyhow::{Context, Result};
use git2::{Repository, BranchType};
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum WorktreeError {
    #[error("Git repository error: {0}")]
    Git(#[from] git2::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Worktree already exists: {0}")]
    AlreadyExists(String),
    #[error("Worktree not found: {0}")]
    NotFound(String),
    #[error("Invalid branch name: {0}")]
    InvalidBranchName(String),
    #[error("Command execution failed: {0}")]
    CommandFailed(String),
}

#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub id: Uuid,
    pub path: PathBuf,
    pub session_path: PathBuf,  // New: symlink path for session-based lookup
    pub branch_name: String,
    pub source_repository: PathBuf,
    pub commit_hash: Option<String>,
}

pub struct WorktreeManager {
    base_worktree_dir: PathBuf,
}

impl WorktreeManager {
    pub fn new() -> Result<Self> {
        let home_dir = dirs::home_dir()
            .context("Failed to get home directory")?;
        let base_dir = home_dir.join(".claude-in-a-box").join("worktrees");

        std::fs::create_dir_all(&base_dir)
            .with_context(|| format!("Failed to create worktree directory: {}", base_dir.display()))?;

        // Create subdirectories for organized storage
        std::fs::create_dir_all(&base_dir.join("by-session"))?;
        std::fs::create_dir_all(&base_dir.join("by-name"))?;

        Ok(Self {
            base_worktree_dir: base_dir,
        })
    }

    pub fn with_base_dir(base_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&base_dir)
            .with_context(|| format!("Failed to create worktree directory: {}", base_dir.display()))?;

        Ok(Self {
            base_worktree_dir: base_dir,
        })
    }

    pub fn create_worktree(
        &self,
        session_id: Uuid,
        repository_path: &Path,
        branch_name: &str,
        base_branch: Option<&str>,
    ) -> Result<WorktreeInfo, WorktreeError> {
        info!("Creating worktree for session {} with branch {}", session_id, branch_name);

        self.validate_branch_name(branch_name)?;

        let repo = Repository::open(repository_path)?;
        let worktree_path = self.generate_worktree_path(session_id, repository_path, branch_name)?;

        // Check if worktree already exists
        if worktree_path.exists() {
            return Err(WorktreeError::AlreadyExists(worktree_path.display().to_string()));
        }

        // Determine the base branch
        let base_branch = base_branch.map(|s| s.to_string()).unwrap_or_else(|| self.get_default_branch(&repo));

        // Create the branch if it doesn't exist
        self.ensure_branch_exists(&repo, branch_name, &base_branch)?;

        // Use git command to create worktree (more reliable than git2 for worktrees)
        self.create_worktree_command(repository_path, &worktree_path, branch_name)?;

        // Create session-based symlink for easy lookup
        let session_path = self.base_worktree_dir.join("by-session").join(session_id.to_string());
        self.create_session_symlink(&worktree_path, &session_path)?;

        let commit_hash = self.get_current_commit_hash(&worktree_path)?;

        let worktree_info = WorktreeInfo {
            id: session_id,
            path: worktree_path,
            session_path,
            branch_name: branch_name.to_string(),
            source_repository: repository_path.to_path_buf(),
            commit_hash,
        };

        info!("Successfully created worktree at: {}", worktree_info.path.display());
        Ok(worktree_info)
    }

    pub fn remove_worktree(&self, session_id: Uuid) -> Result<(), WorktreeError> {
        info!("Removing worktree for session {}", session_id);

        // Find the actual worktree path (it might be in by-name directory)
        let session_path = self.base_worktree_dir.join("by-session").join(session_id.to_string());
        let worktree_path = if session_path.exists() && session_path.is_symlink() {
            std::fs::read_link(&session_path)?
        } else {
            // Fallback to old location for backward compatibility
            self.base_worktree_dir.join(session_id.to_string())
        };

        if !worktree_path.exists() {
            return Err(WorktreeError::NotFound(worktree_path.display().to_string()));
        }

        // Get the original repository path to remove worktree properly
        if let Ok(repo) = Repository::open(&worktree_path) {
            if repo.workdir().is_some() {
                let main_repo_path = self.find_main_repository(&repo)?;

                // Use git command to remove worktree
                self.remove_worktree_command(&main_repo_path, &worktree_path)?;
            }
        } else {
            // If we can't open as repo, just remove the directory
            std::fs::remove_dir_all(&worktree_path)?;
        }

        // Remove the session symlink if it exists
        if session_path.exists() {
            std::fs::remove_file(&session_path)?;
        }

        info!("Successfully removed worktree: {}", worktree_path.display());
        Ok(())
    }

    pub fn list_worktrees(&self) -> Result<Vec<WorktreeInfo>> {
        let mut worktrees = Vec::new();

        if !self.base_worktree_dir.exists() {
            return Ok(worktrees);
        }

        let entries = std::fs::read_dir(&self.base_worktree_dir)?;

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                if let Ok(session_id) = Uuid::parse_str(dir_name) {
                    if let Ok(worktree_info) = self.get_worktree_info(session_id) {
                        worktrees.push(worktree_info);
                    }
                }
            }
        }

        Ok(worktrees)
    }

    pub fn get_worktree_info(&self, session_id: Uuid) -> Result<WorktreeInfo, WorktreeError> {
        // Find the actual worktree path (might be in by-name directory)
        let session_path = self.base_worktree_dir.join("by-session").join(session_id.to_string());
        let worktree_path = if session_path.exists() && session_path.is_symlink() {
            std::fs::read_link(&session_path)?
        } else {
            // Fallback to old location for backward compatibility
            let old_path = self.base_worktree_dir.join(session_id.to_string());
            if old_path.exists() {
                old_path
            } else {
                return Err(WorktreeError::NotFound(format!("Session {} worktree not found", session_id)));
            }
        };

        if !worktree_path.exists() {
            return Err(WorktreeError::NotFound(worktree_path.display().to_string()));
        }

        let repo = Repository::open(&worktree_path)?;
        let head = repo.head()?;
        let branch_name = head.shorthand().unwrap_or("unknown").to_string();
        let commit_hash = self.get_current_commit_hash(&worktree_path)?;

        // Find the source repository
        let source_repository = self.find_main_repository(&repo)?;

        Ok(WorktreeInfo {
            id: session_id,
            path: worktree_path,
            session_path,
            branch_name,
            source_repository,
            commit_hash,
        })
    }

    fn validate_branch_name(&self, name: &str) -> Result<(), WorktreeError> {
        if name.is_empty() {
            return Err(WorktreeError::InvalidBranchName("Branch name cannot be empty".to_string()));
        }

        // Git branch name validation rules
        let invalid_chars = [' ', '~', '^', ':', '?', '*', '[', '\\'];
        if name.chars().any(|c| invalid_chars.contains(&c)) {
            return Err(WorktreeError::InvalidBranchName(
                format!("Branch name contains invalid characters: {}", name)
            ));
        }

        if name.starts_with('-') || name.ends_with('/') || name.contains("//") {
            return Err(WorktreeError::InvalidBranchName(
                format!("Invalid branch name format: {}", name)
            ));
        }

        Ok(())
    }

    fn get_default_branch(&self, repo: &Repository) -> String {
        // Try to find the default branch (main or master)
        if repo.find_branch("main", BranchType::Local).is_ok() {
            "main".to_string()
        } else if repo.find_branch("master", BranchType::Local).is_ok() {
            "master".to_string()
        } else {
            // If neither exists, try to get the current HEAD
            if let Ok(head) = repo.head() {
                if let Some(name) = head.shorthand() {
                    return name.to_string();
                }
            }
            "main".to_string() // Default fallback
        }
    }

    fn ensure_branch_exists(&self, repo: &Repository, branch_name: &str, base_branch: &str) -> Result<(), WorktreeError> {
        // Check if branch already exists
        if repo.find_branch(branch_name, BranchType::Local).is_ok() {
            debug!("Branch {} already exists", branch_name);
            return Ok(());
        }

        // Get the base branch commit
        let base_branch_ref = repo.find_branch(base_branch, BranchType::Local)?;
        let base_commit = base_branch_ref.get().peel_to_commit()?;

        // Create the new branch
        repo.branch(branch_name, &base_commit, false)?;
        info!("Created new branch: {} from {}", branch_name, base_branch);

        Ok(())
    }

    fn create_worktree_command(&self, repo_path: &Path, worktree_path: &Path, branch_name: &str) -> Result<(), WorktreeError> {
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(["worktree", "add", worktree_path.to_str().unwrap(), branch_name])
            .output()?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(WorktreeError::CommandFailed(
                format!("Failed to create worktree: {}", error)
            ));
        }

        Ok(())
    }

    fn remove_worktree_command(&self, repo_path: &Path, worktree_path: &Path) -> Result<(), WorktreeError> {
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(["worktree", "remove", worktree_path.to_str().unwrap()])
            .output()?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            warn!("Git worktree remove failed, trying force remove: {}", error);

            // Try force remove
            let force_output = Command::new("git")
                .current_dir(repo_path)
                .args(["worktree", "remove", "--force", worktree_path.to_str().unwrap()])
                .output()?;

            if !force_output.status.success() {
                return Err(WorktreeError::CommandFailed(
                    format!("Failed to remove worktree: {}", String::from_utf8_lossy(&force_output.stderr))
                ));
            }
        }

        Ok(())
    }

    fn get_current_commit_hash(&self, worktree_path: &Path) -> Result<Option<String>, WorktreeError> {
        let repo = Repository::open(worktree_path)?;
        
        let head_result = repo.head();
        match head_result {
            Ok(head) => {
                if let Some(oid) = head.target() {
                    Ok(Some(oid.to_string()))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }

    fn find_main_repository(&self, worktree_repo: &Repository) -> Result<PathBuf, WorktreeError> {
        // For worktrees, the .git file contains a path to the main repository
        if let Ok(git_dir) = worktree_repo.path().parent().ok_or_else(|| {
            WorktreeError::CommandFailed("Cannot find parent of git directory".to_string())
        }) {
            let git_file = git_dir.join(".git");
            if git_file.is_file() {
                if let Ok(content) = std::fs::read_to_string(&git_file) {
                    // Parse "gitdir: /path/to/main/repo/.git/worktrees/name"
                    if let Some(gitdir_line) = content.lines().find(|line| line.starts_with("gitdir:")) {
                        if let Some(gitdir_path) = gitdir_line.strip_prefix("gitdir:").map(|s| s.trim()) {
                            // Extract main repo path from worktree path
                            let path = PathBuf::from(gitdir_path);
                            if let Some(worktrees_parent) = path.parent().and_then(|p| p.parent()) {
                                return Ok(worktrees_parent.to_path_buf());
                            }
                        }
                    }
                }
            }
        }

        Err(WorktreeError::CommandFailed("Cannot determine main repository path".to_string()))
    }

    fn generate_worktree_path(&self, session_id: Uuid, repository_path: &Path, branch_name: &str) -> Result<PathBuf, WorktreeError> {
        // Extract repository name from path
        let repo_name = repository_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown-repo");

        // Sanitize names for filesystem safety
        let safe_repo_name = self.sanitize_name(repo_name);
        let safe_branch_name = self.sanitize_name(branch_name);
        
        // Generate short UUID for uniqueness (first 8 chars)
        let short_uuid = session_id.to_string()[..8].to_string();
        
        // Create human-readable directory name
        let dir_name = format!("{}--{}--{}", safe_repo_name, safe_branch_name, short_uuid);
        let named_path = self.base_worktree_dir.join("by-name").join(&dir_name);
        
        // Create session symlink path
        let session_path = self.base_worktree_dir.join("by-session").join(session_id.to_string());
        
        // Store both paths in the WorktreeInfo for later cleanup
        // For now, return the named path as the primary path
        Ok(named_path)
    }

    fn sanitize_name(&self, name: &str) -> String {
        name.chars()
            .map(|c| match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => c,
                _ => '-',
            })
            .collect::<String>()
            .trim_matches('-')
            .to_string()
    }

    fn create_session_symlink(&self, worktree_path: &Path, session_path: &Path) -> Result<(), WorktreeError> {
        // Ensure the by-session directory exists
        if let Some(parent) = session_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Remove existing symlink if it exists
        if session_path.exists() {
            std::fs::remove_file(session_path)?;
        }

        // Create symlink based on platform
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(worktree_path, session_path)
                .map_err(|e| WorktreeError::Io(e))?;
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_dir(worktree_path, session_path)
                .map_err(|e| WorktreeError::Io(e))?;
        }
        
        Ok(())
    }
}

impl Default for WorktreeManager {
    fn default() -> Self {
        Self::new().expect("Failed to create default WorktreeManager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_repo(path: &Path) -> Result<Repository> {
        let repo = Repository::init(path)?;
        
        // Create initial commit
        let signature = git2::Signature::now("Test User", "test@example.com")?;
        let tree_id = {
            let mut index = repo.index()?;
            index.write_tree()?
        };
        let tree = repo.find_tree(tree_id)?;
        
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        )?;

        // Drop tree before returning repo
        drop(tree);
        Ok(repo)
    }

    #[test]
    fn test_validate_branch_name() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorktreeManager::with_base_dir(temp_dir.path().to_path_buf()).unwrap();

        assert!(manager.validate_branch_name("valid-branch").is_ok());
        assert!(manager.validate_branch_name("feature/test").is_ok());
        assert!(manager.validate_branch_name("").is_err());
        assert!(manager.validate_branch_name("invalid branch").is_err());
        assert!(manager.validate_branch_name("invalid~branch").is_err());
    }

    #[test]
    fn test_get_default_branch() {
        let temp_dir = TempDir::new().unwrap();
        let repo = create_test_repo(temp_dir.path()).unwrap();
        let manager = WorktreeManager::with_base_dir(temp_dir.path().to_path_buf()).unwrap();

        let default_branch = manager.get_default_branch(&repo);
        assert!(!default_branch.is_empty());
    }

    #[test]
    fn test_worktree_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorktreeManager::with_base_dir(temp_dir.path().to_path_buf());
        
        assert!(manager.is_ok());
        assert!(temp_dir.path().exists());
    }

    #[test]
    fn test_list_empty_worktrees() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorktreeManager::with_base_dir(temp_dir.path().to_path_buf()).unwrap();
        
        let worktrees = manager.list_worktrees().unwrap();
        assert!(worktrees.is_empty());
    }

    #[test]
    fn test_generate_worktree_path() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorktreeManager::with_base_dir(temp_dir.path().to_path_buf()).unwrap();
        
        let session_id = uuid::Uuid::new_v4();
        let repo_path = std::path::Path::new("/home/user/projects/my-awesome-project");
        let branch_name = "feature/user-auth";
        
        let worktree_path = manager.generate_worktree_path(session_id, repo_path, branch_name).unwrap();
        
        // Should be in by-name directory
        assert!(worktree_path.to_string_lossy().contains("by-name"));
        
        // Should contain sanitized repo name
        assert!(worktree_path.to_string_lossy().contains("my-awesome-project"));
        
        // Should contain sanitized branch name (/ becomes -)
        assert!(worktree_path.to_string_lossy().contains("feature-user-auth"));
        
        // Should contain short UUID
        let short_uuid = &session_id.to_string()[..8];
        assert!(worktree_path.to_string_lossy().contains(short_uuid));
        
        println!("Generated worktree path: {}", worktree_path.display());
    }

    #[test]
    fn test_hybrid_path_structure() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorktreeManager::with_base_dir(temp_dir.path().to_path_buf()).unwrap();
        
        let session_id = uuid::Uuid::new_v4();
        let repo_path = std::path::Path::new("/home/user/projects/test-repo");
        let branch_name = "main";
        
        // Test path generation
        let worktree_path = manager.generate_worktree_path(session_id, repo_path, branch_name).unwrap();
        
        // Verify by-name path structure
        assert!(worktree_path.to_string_lossy().contains("by-name"));
        assert!(worktree_path.to_string_lossy().contains("test-repo"));
        assert!(worktree_path.to_string_lossy().contains("main"));
        
        // Verify session path would be created
        let session_path = manager.base_worktree_dir.join("by-session").join(session_id.to_string());
        assert!(session_path.to_string_lossy().contains("by-session"));
        assert!(session_path.to_string_lossy().contains(&session_id.to_string()));
        
        println!("Named path: {}", worktree_path.display());
        println!("Session path: {}", session_path.display());
    }
}