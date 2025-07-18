// ABOUTME: Docker integration for managing development containers

pub mod builder;
pub mod claude_dev;
pub mod claude_dev_tests;
pub mod container_manager;
pub mod session_container;
pub mod session_lifecycle;

pub use builder::{ImageBuilder, BuildOptions};
pub use claude_dev::{ClaudeDevManager, ClaudeDevConfig, ClaudeDevProgress, AuthenticationStatus, create_claude_dev_session};
pub use container_manager::{ContainerManager, ContainerError, RunOptions};
pub use session_container::{SessionContainer, ContainerConfig, ContainerStatus};
pub use session_lifecycle::{SessionLifecycleManager, SessionLifecycleError, SessionRequest, SessionState};