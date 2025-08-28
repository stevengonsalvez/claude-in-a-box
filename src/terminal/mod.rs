// ABOUTME: Terminal module for interactive PTY streaming and terminal emulation
// Provides WebSocket-based direct connection to container PTY service

pub mod interactive_terminal;
pub mod protocol;
pub mod terminal_emulator;
pub mod websocket_client;

pub use interactive_terminal::{InteractiveTerminalComponent, ViewMode};
pub use protocol::{ConnectionState, ConnectionStatus, Message};
pub use terminal_emulator::TerminalEmulatorWidget;
pub use websocket_client::WebSocketTerminalClient;
