// ABOUTME: Terminal module for interactive PTY streaming and terminal emulation
// Provides WebSocket-based direct connection to container PTY service

pub mod protocol;
pub mod websocket_client;
pub mod terminal_emulator;
pub mod interactive_terminal;

pub use interactive_terminal::{InteractiveTerminalComponent, ViewMode};
pub use protocol::{Message, ConnectionState, ConnectionStatus};
pub use websocket_client::WebSocketTerminalClient;
pub use terminal_emulator::TerminalEmulatorWidget;