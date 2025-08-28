// ABOUTME: WebSocket protocol definitions for PTY communication between TUI and container
// Rust counterpart to the TypeScript websocket-protocol.ts in the container

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ============================================
// Base Message Types
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    // Client → PTY Service
    Input(InputMessage),
    Resize(ResizeMessage),
    Signal(SignalMessage),
    PermissionResponse(PermissionResponseMessage),
    Reset(ResetMessage),
    SessionStatus(SessionStatusRequest),
    Heartbeat(HeartbeatMessage),

    // PTY Service → Client
    Output(OutputMessage),
    SessionInit(SessionInitMessage),
    PermissionRequired(PermissionRequiredMessage),
    SessionEnded(SessionEndedMessage),
    SessionReset(SessionResetMessage),
    Error(ErrorMessage),
    SessionStatusResponse(SessionStatusResponse),
    HeartbeatResponse(HeartbeatResponse),
}

// ============================================
// Client → PTY Service Messages
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputMessage {
    pub data: String,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResizeMessage {
    pub cols: u16,
    pub rows: u16,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalMessage {
    pub signal: String, // 'SIGINT', 'SIGTERM', etc
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionResponseMessage {
    pub response: String, // Either number or text option
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetMessage {
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStatusRequest {
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatMessage {
    pub timestamp: i64,
}

// ============================================
// PTY Service → Client Messages
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputMessage {
    pub data: String, // Raw PTY output with ANSI codes
    pub parsed: ParsedOutput,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedOutput {
    pub thinking: Vec<String>, // Internal monologue lines
    pub tool_use: Vec<String>, // Tool usage lines (renamed from toolUse)
    pub content: Vec<String>,  // Actual response content
    pub ui: Vec<String>,       // UI chrome elements
    pub raw: String,           // Original raw output
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInitMessage {
    pub session_id: String,         // renamed from sessionId
    pub buffer: Vec<OutputMessage>, // Recent session history
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequiredMessage {
    pub question: String,
    pub options: Vec<String>,
    pub options_map: HashMap<String, String>, // renamed from optionsMap
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_content: Option<String>, // renamed from planContent
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEndedMessage {
    pub exit_code: i32, // renamed from exitCode
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResetMessage {
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStatusResponse {
    pub session_id: String,   // renamed from sessionId
    pub session_active: bool, // renamed from sessionActive
    pub is_processing: bool,  // renamed from isProcessing
    pub queue_length: usize,  // renamed from queueLength
    pub last_activity: i64,   // renamed from lastActivity
    pub health: HealthInfo,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthInfo {
    pub buffer_size: usize, // renamed from bufferSize
    pub clients: usize,
    pub uptime: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    pub timestamp: i64,
}

// ============================================
// Connection State
// ============================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
    Error,
}

#[derive(Debug, Clone)]
pub struct ConnectionStatus {
    pub state: ConnectionState,
    pub session_id: Option<String>,
    pub last_error: Option<String>,
    pub reconnect_attempts: u32,
}

// ============================================
// Helper functions
// ============================================

impl Message {
    /// Create an input message
    pub fn input(data: String) -> Self {
        Message::Input(InputMessage {
            data,
            timestamp: chrono::Utc::now().timestamp_millis(),
            message_id: None,
        })
    }

    /// Create a resize message
    pub fn resize(cols: u16, rows: u16) -> Self {
        Message::Resize(ResizeMessage {
            cols,
            rows,
            timestamp: chrono::Utc::now().timestamp_millis(),
        })
    }

    /// Create a permission response message
    pub fn permission_response(response: String) -> Self {
        Message::PermissionResponse(PermissionResponseMessage {
            response,
            timestamp: chrono::Utc::now().timestamp_millis(),
        })
    }

    /// Create a reset message
    pub fn reset() -> Self {
        Message::Reset(ResetMessage {
            timestamp: chrono::Utc::now().timestamp_millis(),
        })
    }

    /// Create a heartbeat message
    pub fn heartbeat() -> Self {
        Message::Heartbeat(HeartbeatMessage {
            timestamp: chrono::Utc::now().timestamp_millis(),
        })
    }

    /// Create a session status request
    pub fn session_status() -> Self {
        Message::SessionStatus(SessionStatusRequest {
            timestamp: chrono::Utc::now().timestamp_millis(),
        })
    }
}

// Type guards for pattern matching
impl Message {
    pub fn is_output(&self) -> bool {
        matches!(self, Message::Output(_))
    }

    pub fn is_permission_required(&self) -> bool {
        matches!(self, Message::PermissionRequired(_))
    }

    pub fn is_session_ended(&self) -> bool {
        matches!(self, Message::SessionEnded(_))
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Message::Error(_))
    }
}
