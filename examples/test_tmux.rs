// ABOUTME: Simple test program to verify tmux session management
// Tests creating, attaching, and detaching from tmux sessions

use claude_box::tmux::TmuxSession;
use std::collections::HashMap;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing Tmux Integration");
    
    // Check if tmux is installed
    TmuxSession::check_tmux_installed()?;
    println!("✓ Tmux is installed");
    
    // Create a test session
    let mut env_vars = HashMap::new();
    env_vars.insert("TEST_VAR".to_string(), "test_value".to_string());
    
    let mut session = TmuxSession::create(
        "test_session",
        "/tmp",
        "/bin/bash",
        &env_vars,
    ).await?;
    
    println!("✓ Created tmux session: {}", session.name);
    
    // List sessions
    let sessions = TmuxSession::list_sessions().await?;
    println!("✓ Found {} CIAB sessions", sessions.len());
    for s in &sessions {
        println!("  - {}", s);
    }
    
    // Capture pane content
    let content = session.capture_pane().await?;
    println!("✓ Captured pane content ({} bytes)", content.len());
    
    // Test window resize
    session.resize(100, 30)?;
    println!("✓ Resized window to 100x30");
    
    // Clean up
    session.kill().await?;
    println!("✓ Killed tmux session");
    
    println!("\nAll tests passed!");
    Ok(())
}