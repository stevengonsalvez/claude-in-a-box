// ABOUTME: Core data models for Claude-in-a-Box sessions, workspaces, and state management

pub mod session;
pub mod workspace;

pub use session::{GitChanges, Session, SessionMode, SessionStatus};
pub use workspace::Workspace;
