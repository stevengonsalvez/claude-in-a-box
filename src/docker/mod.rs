// ABOUTME: Docker integration for managing development containers

pub mod builder;
pub mod claude_dev;
pub mod claude_dev_tests;
pub mod container_manager;
pub mod log_streaming;
pub mod session_container;
pub mod session_lifecycle;
pub mod session_progress;

pub use builder::ImageBuilder;
pub use claude_dev::{ClaudeDevConfig, ClaudeDevProgress, create_claude_dev_session};
pub use container_manager::{ContainerError, ContainerManager};
pub use log_streaming::LogStreamingCoordinator;
pub use session_container::{ContainerConfig, ContainerStatus, SessionContainer};
pub use session_lifecycle::SessionLifecycleManager;
pub use session_progress::SessionProgress;
