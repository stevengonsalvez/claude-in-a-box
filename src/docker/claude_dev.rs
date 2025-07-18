// ABOUTME: Claude-dev container management module
// Handles authentication, environment setup, and container operations for claude-dev sessions

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::builder::ImageBuilder;
use super::container_manager::ContainerManager;

/// Configuration for claude-dev container setup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeDevConfig {
    /// Container image name
    pub image_name: String,
    /// Memory limit (e.g., "4g", "2048m")
    pub memory_limit: Option<String>,
    /// GPU access (e.g., "all", "device=0")
    pub gpu_access: Option<String>,
    /// Whether to force rebuild image
    pub force_rebuild: bool,
    /// Whether to build without cache
    pub no_cache: bool,
    /// Whether to continue from last session
    pub continue_session: bool,
    /// Environment variables to pass to container
    pub env_vars: HashMap<String, String>,
}

impl Default for ClaudeDevConfig {
    fn default() -> Self {
        Self {
            image_name: "claude-box:claude-dev".to_string(),
            memory_limit: None,
            gpu_access: None,
            force_rebuild: false,
            no_cache: false,
            continue_session: false,
            env_vars: HashMap::new(),
        }
    }
}

/// Authentication status for claude-dev
#[derive(Debug, Clone)]
pub struct AuthenticationStatus {
    pub claude_json_exists: bool,
    pub credentials_json_exists: bool,
    pub anthropic_api_key_set: bool,
    pub github_token_set: bool,
    pub sources: Vec<String>,
}

/// Progress updates for claude-dev operations
#[derive(Debug, Clone)]
pub enum ClaudeDevProgress {
    SyncingAuthentication,
    CheckingEnvironment,
    BuildingImage(String),
    StartingContainer,
    ConfiguringGitHub,
    Ready,
    Error(String),
}

/// Main claude-dev container manager
pub struct ClaudeDevManager {
    config: ClaudeDevConfig,
    container_manager: ContainerManager,
    image_builder: ImageBuilder,
    claude_home_dir: PathBuf,
    ssh_dir: PathBuf,
}

impl ClaudeDevManager {
    /// Create new claude-dev manager
    pub async fn new(config: ClaudeDevConfig) -> Result<Self> {
        let container_manager = ContainerManager::new().await?;
        let image_builder = ImageBuilder::new().await?;
        
        // Setup claude-box directories
        let home_dir = dirs::home_dir().context("Failed to get home directory")?;
        let claude_home_dir = home_dir.join(".claude-box").join("claude-home");
        let ssh_dir = home_dir.join(".claude-box").join("ssh");
        
        // Ensure directories exist
        std::fs::create_dir_all(&claude_home_dir)?;
        std::fs::create_dir_all(&ssh_dir)?;
        
        Ok(Self {
            config,
            container_manager,
            image_builder,
            claude_home_dir,
            ssh_dir,
        })
    }

    /// Get authentication status
    pub fn get_authentication_status(&self) -> Result<AuthenticationStatus> {
        let home_dir = dirs::home_dir().context("Failed to get home directory")?;
        let mut sources = Vec::new();
        
        // Check for .claude.json in persistent directory
        let claude_json_path = self.claude_home_dir.join(".claude.json");
        let claude_json_exists = claude_json_path.exists() && claude_json_path.metadata()?.len() > 0;
        if claude_json_exists {
            sources.push(".claude.json (persistent)".to_string());
        }
        
        // Check for .credentials.json in persistent directory
        let credentials_path = self.claude_home_dir.join(".credentials.json");
        let credentials_json_exists = credentials_path.exists() && credentials_path.metadata()?.len() > 0;
        if credentials_json_exists {
            sources.push(".credentials.json (persistent)".to_string());
        }
        
        // Check for environment variables
        let anthropic_api_key_set = std::env::var("ANTHROPIC_API_KEY").is_ok();
        if anthropic_api_key_set {
            sources.push("ANTHROPIC_API_KEY environment variable".to_string());
        }
        
        let github_token_set = std::env::var("GITHUB_TOKEN").is_ok() || 
                               self.config.env_vars.contains_key("GITHUB_TOKEN");
        if github_token_set {
            sources.push("GITHUB_TOKEN environment variable".to_string());
        }
        
        Ok(AuthenticationStatus {
            claude_json_exists,
            credentials_json_exists,
            anthropic_api_key_set,
            github_token_set,
            sources,
        })
    }

    /// Sync authentication files from host to persistent directory
    pub async fn sync_authentication_files(&self, progress_tx: Option<mpsc::Sender<ClaudeDevProgress>>) -> Result<()> {
        if let Some(ref tx) = progress_tx {
            let _ = tx.send(ClaudeDevProgress::SyncingAuthentication).await;
        }
        
        let home_dir = dirs::home_dir().context("Failed to get home directory")?;
        let mut sync_needed = false;
        
        // Check if we need to sync .claude.json
        let host_claude_json = home_dir.join(".claude.json");
        let persistent_claude_json = self.claude_home_dir.join(".claude.json");
        
        if host_claude_json.exists() {
            if !persistent_claude_json.exists() || 
               self.is_newer(&host_claude_json, &persistent_claude_json)? {
                sync_needed = true;
            }
        }
        
        // Check if we need to sync .claude directory
        let host_claude_dir = home_dir.join(".claude");
        if host_claude_dir.exists() {
            if !self.claude_home_dir.exists() ||
               !self.claude_home_dir.join(".credentials.json").exists() ||
               self.is_newer(&host_claude_dir, &self.claude_home_dir)? {
                sync_needed = true;
            }
        }
        
        if sync_needed {
            info!("Syncing Claude configuration to persistent directory");
            
            // Sync .claude.json if it exists
            if host_claude_json.exists() {
                tokio::fs::copy(&host_claude_json, &persistent_claude_json).await
                    .context("Failed to copy .claude.json")?;
                debug!("Copied .claude.json to persistent directory");
            }
            
            // Sync .claude directory contents if they exist
            if host_claude_dir.exists() {
                self.sync_directory(&host_claude_dir, &self.claude_home_dir).await?;
                debug!("Synced .claude directory to persistent directory");
            }
        }
        
        Ok(())
    }

    /// Setup environment variables and GitHub CLI configuration
    pub async fn setup_environment(&self, progress_tx: Option<mpsc::Sender<ClaudeDevProgress>>) -> Result<()> {
        if let Some(ref tx) = progress_tx {
            let _ = tx.send(ClaudeDevProgress::CheckingEnvironment).await;
        }
        
        // Check for GITHUB_TOKEN
        let github_token = std::env::var("GITHUB_TOKEN")
            .or_else(|_| self.config.env_vars.get("GITHUB_TOKEN").cloned().ok_or_else(|| std::env::VarError::NotPresent));
        
        if let Ok(token) = github_token {
            info!("GITHUB_TOKEN found - will use token-based authentication");
            debug!("GitHub CLI and token-based git operations will be available");
        } else {
            warn!("GITHUB_TOKEN not found");
            info!("To enable full GitHub integration:");
            info!("  1. Create GitHub Personal Access Token:");
            info!("     https://github.com/settings/tokens/new");
            info!("     Required scopes: repo, read:org, workflow");
            info!("  2. Set GITHUB_TOKEN environment variable");
            
            // Check for SSH keys as fallback
            let ssh_key_path = self.ssh_dir.join("id_rsa");
            let ssh_pub_key_path = self.ssh_dir.join("id_rsa.pub");
            
            if ssh_key_path.exists() && ssh_pub_key_path.exists() {
                info!("SSH keys found as fallback for git operations");
                self.setup_ssh_config().await?;
            } else {
                info!("Alternative: Generate SSH keys:");
                info!("  ssh-keygen -t rsa -b 4096 -f ~/.claude-box/ssh/id_rsa -N ''");
                info!("  Then add public key to GitHub/GitLab");
                info!("Note: GITHUB_TOKEN is recommended for better integration");
            }
        }
        
        Ok(())
    }

    /// Build claude-dev Docker image if needed
    pub async fn build_image_if_needed(&self, progress_tx: Option<mpsc::Sender<ClaudeDevProgress>>) -> Result<()> {
        let need_rebuild = self.config.force_rebuild || 
                          !self.image_exists(&self.config.image_name).await?;
        
        if need_rebuild {
            if let Some(ref tx) = progress_tx {
                let _ = tx.send(ClaudeDevProgress::BuildingImage("Starting build...".to_string())).await;
            }
            
            info!("Building claude-dev image: {}", self.config.image_name);
            
            // Get current user UID/GID
            let uid = nix::unistd::getuid().as_raw();
            let gid = nix::unistd::getgid().as_raw();
            
            // Build arguments
            let mut build_args = vec![
                ("HOST_UID".to_string(), uid.to_string()),
                ("HOST_GID".to_string(), gid.to_string()),
            ];
            
            // Add environment variables if they exist
            if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
                build_args.push(("ANTHROPIC_API_KEY".to_string(), api_key));
            }
            
            // Build the image
            let dockerfile_dir = PathBuf::from("docker/claude-dev");
            let build_options = super::builder::BuildOptions {
                dockerfile_path: Some(dockerfile_dir.join("Dockerfile")),
                context_path: dockerfile_dir,
                build_args,
                no_cache: self.config.no_cache,
                target: None,
                labels: vec![],
                pull: false,
            };
            
            // Create progress sender for image build
            let (build_tx, mut build_rx) = mpsc::channel(100);
            let progress_tx_clone = progress_tx.clone();
            
            // Spawn task to forward build progress
            if progress_tx.is_some() {
                tokio::spawn(async move {
                    while let Some(log) = build_rx.recv().await {
                        if let Some(ref tx) = progress_tx_clone {
                            let _ = tx.send(ClaudeDevProgress::BuildingImage(log)).await;
                        }
                    }
                });
            }
            
            self.image_builder.build_image(
                &self.config.image_name,
                &build_options,
                Some(build_tx),
            ).await?;
            
            info!("Successfully built claude-dev image");
        } else {
            debug!("Image {} already exists, skipping build", self.config.image_name);
        }
        
        Ok(())
    }

    /// Run claude-dev container
    pub async fn run_container(&self, workspace_path: &Path, session_id: Uuid, progress_tx: Option<mpsc::Sender<ClaudeDevProgress>>) -> Result<String> {
        if let Some(ref tx) = progress_tx {
            let _ = tx.send(ClaudeDevProgress::StartingContainer).await;
        }
        
        info!("Starting claude-dev container");
        info!("Container: {}", self.config.image_name);
        info!("Workspace: {}", workspace_path.display());
        
        // Prepare container configuration
        let mut env_vars = self.config.env_vars.clone();
        env_vars.insert("CLAUDE_BOX_MODE".to_string(), "true".to_string());
        
        // Add continue flag if requested
        if self.config.continue_session {
            env_vars.insert("CLAUDE_CONTINUE_FLAG".to_string(), "--continue".to_string());
        }
        
        // Setup volume mounts
        let mut mounts = vec![
            // Workspace mount
            (workspace_path.to_path_buf(), PathBuf::from("/workspace")),
            // Claude home directory
            (self.claude_home_dir.clone(), PathBuf::from("/home/claude-user/.claude")),
            // SSH directory
            (self.ssh_dir.clone(), PathBuf::from("/home/claude-user/.ssh")),
        ];
        
        // Mount .claude.json separately if it exists
        let claude_json_path = self.claude_home_dir.join(".claude.json");
        if claude_json_path.exists() {
            mounts.push((claude_json_path, PathBuf::from("/home/claude-user/.claude.json")));
            info!("Mounting .claude.json for authentication");
        }
        
        // Container run options
        let mut labels = std::collections::HashMap::new();
        labels.insert("claude-session-id".to_string(), session_id.to_string());
        
        let run_options = super::container_manager::RunOptions {
            image: self.config.image_name.clone(),
            command: vec![],
            env_vars,
            mounts,
            working_dir: Some("/workspace".to_string()),
            user: None,
            network: None,
            ports: vec![],
            remove_on_exit: true,
            interactive: true,
            tty: true,
            memory_limit: self.config.memory_limit.clone(),
            cpu_limit: None,
            gpu_access: self.config.gpu_access.clone(),
            labels,
        };
        
        // Generate container name with session ID
        let container_name = format!("claude-session-{}", session_id);
        
        // Run the container
        let container_id = self.container_manager.run_container(&container_name, &run_options).await?;
        
        if let Some(ref tx) = progress_tx {
            let _ = tx.send(ClaudeDevProgress::Ready).await;
        }
        
        info!("Claude-dev container started successfully: {}", container_id);
        Ok(container_id)
    }

    /// Check if Docker image exists
    async fn image_exists(&self, image_name: &str) -> Result<bool> {
        let output = Command::new("docker")
            .args(&["images", "-q", image_name])
            .output()
            .context("Failed to check if image exists")?;
        
        Ok(!output.stdout.is_empty())
    }

    /// Check if first file is newer than second file
    fn is_newer(&self, file1: &Path, file2: &Path) -> Result<bool> {
        if !file2.exists() {
            return Ok(true);
        }
        
        let metadata1 = file1.metadata()?;
        let metadata2 = file2.metadata()?;
        
        Ok(metadata1.modified()? > metadata2.modified()?)
    }

    /// Sync directory contents recursively
    fn sync_directory<'a>(&'a self, source: &'a Path, dest: &'a Path) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            if !dest.exists() {
                tokio::fs::create_dir_all(dest).await?;
            }
            
            let mut entries = tokio::fs::read_dir(source).await?;
            while let Some(entry) = entries.next_entry().await? {
                let file_name = entry.file_name();
                let source_path = entry.path();
                let dest_path = dest.join(&file_name);
                
                if source_path.is_file() {
                    tokio::fs::copy(&source_path, &dest_path).await?;
                } else if source_path.is_dir() {
                    self.sync_directory(&source_path, &dest_path).await?;
                }
            }
            
            Ok(())
        })
    }

    /// Setup SSH configuration
    async fn setup_ssh_config(&self) -> Result<()> {
        let ssh_config_path = self.ssh_dir.join("config");
        if !ssh_config_path.exists() {
            let config_content = r#"Host github.com
    HostName github.com
    User git
    IdentityFile ~/.ssh/id_rsa
    IdentitiesOnly yes

Host gitlab.com
    HostName gitlab.com
    User git
    IdentityFile ~/.ssh/id_rsa
    IdentitiesOnly yes
"#;
            tokio::fs::write(&ssh_config_path, config_content).await?;
            info!("SSH config created");
        }
        Ok(())
    }
}

/// Helper function to create a claude-dev session
pub async fn create_claude_dev_session(
    workspace_path: &Path,
    config: ClaudeDevConfig,
    session_id: Uuid,
    progress_tx: Option<mpsc::Sender<ClaudeDevProgress>>,
) -> Result<String> {
    let manager = ClaudeDevManager::new(config).await?;
    
    // Sync authentication files
    manager.sync_authentication_files(progress_tx.clone()).await?;
    
    // Setup environment
    manager.setup_environment(progress_tx.clone()).await?;
    
    // Build image if needed
    manager.build_image_if_needed(progress_tx.clone()).await?;
    
    // Run container
    let container_id = manager.run_container(workspace_path, session_id, progress_tx).await?;
    
    Ok(container_id)
}