// ABOUTME: Docker container management using Bollard for creating and managing development containers

use super::{SessionContainer, ContainerConfig, ContainerStatus};
use anyhow::Result;
use bollard::container::{
    Config, CreateContainerOptions, LogOutput, LogsOptions, RemoveContainerOptions,
    StartContainerOptions, StopContainerOptions, ListContainersOptions,
};
use bollard::image::{CreateImageOptions, ListImagesOptions};
use bollard::models::{ContainerSummary, HostConfig, HostConfigLogConfig, Mount, MountTypeEnum, PortBinding};
use bollard::Docker;
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, info, warn, error};
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum ContainerError {
    #[error("Docker connection error: {0}")]
    Connection(#[from] bollard::errors::Error),
    #[error("Container not found: {0}")]
    NotFound(String),
    #[error("Container already exists: {0}")]
    AlreadyExists(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Container operation failed: {0}")]
    OperationFailed(String),
}

pub struct ContainerManager {
    docker: Docker,
}

impl ContainerManager {
    pub async fn new() -> Result<Self, ContainerError> {
        let docker = Self::connect_to_docker()
            .map_err(ContainerError::Connection)?;

        // Test the connection
        docker.ping().await.map_err(ContainerError::Connection)?;

        info!("Successfully connected to Docker daemon");
        Ok(Self { docker })
    }

    fn connect_to_docker() -> Result<Docker, bollard::errors::Error> {
        // Try configuration file first
        if let Ok(config) = crate::config::AppConfig::load() {
            if let Some(docker_host) = &config.docker.host {
                info!("Using Docker host from config: {}", docker_host);
                std::env::set_var("DOCKER_HOST", docker_host);
                
                match Docker::connect_with_local_defaults() {
                    Ok(docker) => return Ok(docker),
                    Err(e) => {
                        warn!("Failed to connect to configured Docker host {}: {}", docker_host, e);
                        // Continue with other detection methods
                    }
                }
            }
        }

        // Try environment variable next
        if let Ok(docker_host) = std::env::var("DOCKER_HOST") {
            info!("Using DOCKER_HOST: {}", docker_host);
            return Docker::connect_with_local_defaults();
        }

        // Try common Docker socket locations based on OS
        let socket_paths = Self::get_docker_socket_paths();
        
        for socket_path in socket_paths {
            let exists = if socket_path.starts_with("npipe:") {
                // For Windows named pipes, we can't check existence easily
                // Just try to connect
                true
            } else {
                std::path::Path::new(&socket_path).exists()
            };
            
            if exists {
                info!("Found Docker socket at: {}", socket_path);
                
                // Set DOCKER_HOST environment variable for this process
                let docker_host = if socket_path.starts_with("npipe:") {
                    socket_path.clone()
                } else {
                    format!("unix://{}", socket_path)
                };
                
                std::env::set_var("DOCKER_HOST", docker_host);
                
                match Docker::connect_with_local_defaults() {
                    Ok(docker) => return Ok(docker),
                    Err(e) => {
                        warn!("Failed to connect to Docker socket {}: {}", socket_path, e);
                        continue;
                    }
                }
            }
        }

        // Fall back to default connection
        warn!("No Docker socket found, trying default connection");
        Docker::connect_with_local_defaults()
    }

    fn get_docker_socket_paths() -> Vec<String> {
        let mut paths = Vec::new();

        // Try to get Docker context information first
        if let Some(context_socket) = Self::get_docker_context_socket() {
            paths.push(context_socket);
        }

        // macOS specific paths
        if cfg!(target_os = "macos") {
            // Docker Desktop for Mac
            if let Some(home) = std::env::var("HOME").ok() {
                paths.push(format!("{}/.docker/run/docker.sock", home));
            }
            
            // Colima
            if let Some(home) = std::env::var("HOME").ok() {
                paths.push(format!("{}/.colima/default/docker.sock", home));
            }
            
            // Podman Desktop
            if let Some(home) = std::env::var("HOME").ok() {
                paths.push(format!("{}/.local/share/containers/podman/machine/podman.sock", home));
            }
        }

        // Linux specific paths
        if cfg!(target_os = "linux") {
            // Standard Docker socket
            paths.push("/var/run/docker.sock".to_string());
            
            // Rootless Docker
            if let Ok(xdg_runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
                paths.push(format!("{}/docker.sock", xdg_runtime_dir));
            }
            
            // Podman
            if let Ok(xdg_runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
                paths.push(format!("{}/podman/podman.sock", xdg_runtime_dir));
            }
        }

        // Windows specific paths
        if cfg!(target_os = "windows") {
            // Docker Desktop for Windows (named pipe)
            paths.push("npipe:////./pipe/docker_engine".to_string());
            
            // WSL2 integration
            paths.push("/var/run/docker.sock".to_string());
        }

        paths
    }

    fn get_docker_context_socket() -> Option<String> {
        // Try to get the current Docker context
        let output = std::process::Command::new("docker")
            .args(["context", "inspect", "--format", "{{.Endpoints.docker.Host}}"])
            .output()
            .ok()?;

        if output.status.success() {
            let socket_url = String::from_utf8(output.stdout).ok()?;
            let socket_url = socket_url.trim();
            
            // Extract the socket path from the URL
            if let Some(path) = socket_url.strip_prefix("unix://") {
                debug!("Docker context socket: {}", path);
                return Some(path.to_string());
            } else if socket_url.starts_with("npipe:") {
                debug!("Docker context named pipe: {}", socket_url);
                return Some(socket_url.to_string());
            }
        }
        
        None
    }

    pub async fn create_session_container(
        &self,
        session_id: Uuid,
        config: ContainerConfig,
    ) -> Result<SessionContainer, ContainerError> {
        info!("Creating container for session {}", session_id);

        // Generate a container name
        let container_name = format!("claude-session-{}", session_id);

        // Check if container already exists
        if self.container_exists(&container_name).await? {
            return Err(ContainerError::AlreadyExists(container_name));
        }

        // Ensure image exists
        self.ensure_image_available(&config.image).await?;

        // Create port bindings
        let mut port_bindings = HashMap::new();
        for port_mapping in &config.ports {
            let host_port = port_mapping.host_port
                .map(|p| p.to_string())
                .unwrap_or_else(|| "".to_string()); // Empty string for auto-assignment
            
            let container_port_key = format!("{}/{}", port_mapping.container_port, port_mapping.protocol);
            port_bindings.insert(
                container_port_key,
                Some(vec![PortBinding {
                    host_ip: Some("127.0.0.1".to_string()),
                    host_port: Some(host_port),
                }]),
            );
        }

        // Create volume mounts
        let mut mounts = Vec::new();
        for volume in &config.volumes {
            mounts.push(Mount {
                target: Some(volume.container_path.clone()),
                source: Some(volume.host_path.to_string_lossy().to_string()),
                typ: Some(MountTypeEnum::BIND),
                read_only: Some(volume.read_only),
                consistency: Some("delegated".to_string()), // Better performance on macOS
                ..Default::default()
            });
        }

        // Create host config
        let host_config = HostConfig {
            port_bindings: Some(port_bindings),
            mounts: Some(mounts),
            memory: config.memory_limit.map(|m| m as i64),
            nano_cpus: config.cpu_limit.map(|c| (c * 1_000_000_000.0) as i64),
            auto_remove: Some(false), // We want to manage lifecycle manually
            log_config: Some(HostConfigLogConfig {
                typ: Some("json-file".to_string()),
                config: Some({
                    let mut log_config = HashMap::new();
                    log_config.insert("max-size".to_string(), "10m".to_string());
                    log_config.insert("max-file".to_string(), "3".to_string());
                    log_config
                }),
            }),
            ..Default::default()
        };

        // Prepare environment variables
        let env: Vec<String> = config.environment_vars
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        // Create container config
        let container_config = Config {
            image: Some(config.image.clone()),
            working_dir: Some(config.working_dir.clone()),
            env: Some(env),
            cmd: config.command.clone(),
            entrypoint: config.entrypoint.clone(),
            user: config.user.clone(),
            host_config: Some(host_config),
            labels: Some({
                let mut labels = HashMap::new();
                labels.insert("claude-session-id".to_string(), session_id.to_string());
                labels.insert("claude-managed".to_string(), "true".to_string());
                labels
            }),
            ..Default::default()
        };

        // Create the container
        let create_options = CreateContainerOptions {
            name: container_name.clone(),
            platform: None,
        };

        let create_response = self
            .docker
            .create_container(Some(create_options), container_config)
            .await?;

        info!("Created container {} with ID {}", container_name, create_response.id);

        let mut container = SessionContainer::new(session_id, config);
        container.container_id = Some(create_response.id.clone());
        container.status = ContainerStatus::Stopped;

        // Get the actual port mappings after creation
        container.host_ports = self.get_container_port_mappings(&create_response.id).await?;

        Ok(container)
    }

    pub async fn start_container(&self, container: &mut SessionContainer) -> Result<(), ContainerError> {
        let container_id = container.container_id
            .as_ref()
            .ok_or_else(|| ContainerError::InvalidConfig("No container ID".to_string()))?;

        info!("Starting container {}", container_id);

        container.status = ContainerStatus::Creating;

        self.docker
            .start_container(container_id, None::<StartContainerOptions<String>>)
            .await?;

        // Update port mappings (they might change on restart)
        container.host_ports = self.get_container_port_mappings(container_id).await?;
        container.status = ContainerStatus::Running;
        container.started_at = Some(chrono::Utc::now());

        info!("Successfully started container {}", container_id);
        Ok(())
    }

    pub async fn stop_container(&self, container: &mut SessionContainer) -> Result<(), ContainerError> {
        let container_id = container.container_id
            .as_ref()
            .ok_or_else(|| ContainerError::InvalidConfig("No container ID".to_string()))?;

        info!("Stopping container {}", container_id);

        let stop_options = StopContainerOptions { t: 10 }; // 10 second grace period

        match self.docker.stop_container(container_id, Some(stop_options)).await {
            Ok(_) => {
                container.status = ContainerStatus::Stopped;
                container.finished_at = Some(chrono::Utc::now());
                info!("Successfully stopped container {}", container_id);
                Ok(())
            }
            Err(bollard::errors::Error::DockerResponseServerError { status_code: 304, .. }) => {
                // Container was already stopped
                container.status = ContainerStatus::Stopped;
                debug!("Container {} was already stopped", container_id);
                Ok(())
            }
            Err(e) => Err(ContainerError::Connection(e)),
        }
    }

    pub async fn remove_container(&self, container: &mut SessionContainer) -> Result<(), ContainerError> {
        let container_id = container.container_id
            .as_ref()
            .ok_or_else(|| ContainerError::InvalidConfig("No container ID".to_string()))?
            .clone();

        info!("Removing container {}", container_id);

        // Stop the container first if it's running
        if container.is_running() {
            self.stop_container(container).await?;
        }

        let remove_options = RemoveContainerOptions {
            force: true,
            v: true, // Remove associated volumes
            ..Default::default()
        };

        match self.docker.remove_container(&container_id, Some(remove_options)).await {
            Ok(_) => {
                container.status = ContainerStatus::NotFound;
                container.container_id = None;
                info!("Successfully removed container {}", container_id);
                Ok(())
            }
            Err(bollard::errors::Error::DockerResponseServerError { status_code: 404, .. }) => {
                // Container was already removed
                container.status = ContainerStatus::NotFound;
                container.container_id = None;
                debug!("Container {} was already removed", container_id);
                Ok(())
            }
            Err(e) => Err(ContainerError::Connection(e)),
        }
    }

    pub async fn get_container_status(&self, container_id: &str) -> Result<ContainerStatus, ContainerError> {
        let containers = self
            .docker
            .list_containers(Some(ListContainersOptions::<String> {
                all: true,
                filters: {
                    let mut filters = HashMap::new();
                    filters.insert("id".to_string(), vec![container_id.to_string()]);
                    filters
                },
                ..Default::default()
            }))
            .await?;

        if let Some(container) = containers.first() {
            let status = container.state.as_deref().unwrap_or("unknown");
            match status {
                "running" => Ok(ContainerStatus::Running),
                "paused" => Ok(ContainerStatus::Paused),
                "exited" | "dead" => Ok(ContainerStatus::Stopped),
                "created" => Ok(ContainerStatus::Creating),
                _ => Ok(ContainerStatus::Error(format!("Unknown status: {}", status))),
            }
        } else {
            Ok(ContainerStatus::NotFound)
        }
    }

    pub async fn get_container_logs(
        &self,
        container_id: &str,
        lines: Option<i64>,
    ) -> Result<Vec<String>, ContainerError> {
        let options = LogsOptions::<String> {
            stdout: true,
            stderr: true,
            tail: lines.map(|n| n.to_string()).unwrap_or_else(|| "100".to_string()),
            ..Default::default()
        };

        let mut stream = self.docker.logs(container_id, Some(options));
        let mut logs = Vec::new();

        use futures_util::stream::StreamExt;
        while let Some(log_result) = stream.next().await {
            match log_result {
                Ok(LogOutput::StdOut { message }) | Ok(LogOutput::StdErr { message }) => {
                    if let Ok(log_line) = String::from_utf8(message.to_vec()) {
                        logs.push(log_line.trim_end().to_string());
                    }
                }
                Ok(_) => {} // Ignore other log types
                Err(e) => {
                    warn!("Error reading container logs: {}", e);
                    break;
                }
            }
        }

        Ok(logs)
    }

    pub async fn list_claude_containers(&self) -> Result<Vec<ContainerSummary>, ContainerError> {
        let containers = self
            .docker
            .list_containers(Some(ListContainersOptions::<String> {
                all: true,
                filters: {
                    let mut filters = HashMap::new();
                    filters.insert("label".to_string(), vec!["claude-managed=true".to_string()]);
                    filters
                },
                ..Default::default()
            }))
            .await?;

        Ok(containers)
    }

    async fn container_exists(&self, name: &str) -> Result<bool, ContainerError> {
        let containers = self
            .docker
            .list_containers(Some(ListContainersOptions::<String> {
                all: true,
                filters: {
                    let mut filters = HashMap::new();
                    filters.insert("name".to_string(), vec![name.to_string()]);
                    filters
                },
                ..Default::default()
            }))
            .await?;

        Ok(!containers.is_empty())
    }

    async fn ensure_image_available(&self, image: &str) -> Result<(), ContainerError> {
        // Check if image exists locally
        let images = self
            .docker
            .list_images(Some(ListImagesOptions::<String> {
                filters: {
                    let mut filters = HashMap::new();
                    filters.insert("reference".to_string(), vec![image.to_string()]);
                    filters
                },
                ..Default::default()
            }))
            .await?;

        if !images.is_empty() {
            debug!("Image {} already exists locally", image);
            return Ok(());
        }

        info!("Pulling image {}", image);

        let create_image_options = CreateImageOptions {
            from_image: image,
            ..Default::default()
        };

        use futures_util::stream::StreamExt;
        let mut stream = self.docker.create_image(Some(create_image_options), None, None);

        while let Some(result) = stream.next().await {
            match result {
                Ok(_) => {} // Progress update
                Err(e) => {
                    error!("Failed to pull image {}: {}", image, e);
                    return Err(ContainerError::OperationFailed(format!("Failed to pull image: {}", e)));
                }
            }
        }

        info!("Successfully pulled image {}", image);
        Ok(())
    }

    async fn get_container_port_mappings(&self, container_id: &str) -> Result<HashMap<u16, u16>, ContainerError> {
        let container = self.docker.inspect_container(container_id, None).await?;
        let mut port_mappings = HashMap::new();

        if let Some(network_settings) = container.network_settings {
            if let Some(ports) = network_settings.ports {
                for (container_port_key, host_ports) in ports {
                    if let Some(host_ports) = host_ports {
                        for host_port in host_ports {
                            if let (Ok(container_port), Some(host_port_str)) = (
                                container_port_key.split('/').next().unwrap_or("").parse::<u16>(),
                                &host_port.host_port,
                            ) {
                                if let Ok(host_port) = host_port_str.parse::<u16>() {
                                    port_mappings.insert(container_port, host_port);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(port_mappings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // Note: These tests require Docker to be running
    // They are integration tests and should be run with `cargo test --ignored`

    #[tokio::test]
    #[ignore]
    async fn test_container_manager_creation() {
        let manager = ContainerManager::new().await;
        assert!(manager.is_ok(), "Should be able to connect to Docker");
    }

    #[tokio::test]
    #[ignore]
    async fn test_container_lifecycle() {
        let manager = ContainerManager::new().await.unwrap();
        let session_id = Uuid::new_v4();
        let temp_dir = TempDir::new().unwrap();
        
        let config = ContainerConfig::new("alpine:latest".to_string())
            .with_command(vec!["sleep".to_string(), "30".to_string()])
            .with_volume(temp_dir.path().to_path_buf(), "/workspace".to_string(), false);

        // Create container
        let mut container = manager.create_session_container(session_id, config).await.unwrap();
        assert!(container.container_id.is_some());
        assert_eq!(container.status, ContainerStatus::Stopped);

        // Start container
        manager.start_container(&mut container).await.unwrap();
        assert_eq!(container.status, ContainerStatus::Running);

        // Stop container
        manager.stop_container(&mut container).await.unwrap();
        assert_eq!(container.status, ContainerStatus::Stopped);

        // Remove container
        manager.remove_container(&mut container).await.unwrap();
        assert_eq!(container.status, ContainerStatus::NotFound);
        assert!(container.container_id.is_none());
    }
}