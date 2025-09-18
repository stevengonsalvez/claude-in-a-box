// ABOUTME: Notification system for displaying temporary messages to users
// Provides different types of notifications with automatic expiry

use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub enum NotificationType {
    Success,
    Error,
    Info,
    Warning,
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub message: String,
    pub notification_type: NotificationType,
    pub created_at: Instant,
    pub duration: Duration,
}

impl Notification {
    pub fn new(message: String, notification_type: NotificationType) -> Self {
        Self {
            message,
            notification_type,
            created_at: Instant::now(),
            duration: Duration::from_secs(5), // Default 5 second duration
        }
    }

    pub fn success(message: String) -> Self {
        Self::new(message, NotificationType::Success)
    }

    pub fn error(message: String) -> Self {
        Self::new(message, NotificationType::Error)
    }

    pub fn info(message: String) -> Self {
        Self::new(message, NotificationType::Info)
    }

    pub fn warning(message: String) -> Self {
        Self::new(message, NotificationType::Warning)
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.duration
    }
}