// ABOUTME: Docker integration for managing development containers

pub mod builder;
pub mod container_manager;
pub mod session_container;
pub mod session_lifecycle;

pub use builder::ImageBuilder;
pub use container_manager::{ContainerManager, ContainerError};
pub use session_container::{SessionContainer, ContainerConfig, ContainerStatus};
pub use session_lifecycle::{SessionLifecycleManager, SessionLifecycleError, SessionRequest, SessionState};