// ABOUTME: Simple standalone test for tmux session management
// Tests basic tmux operations without full application dependencies

use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing Tmux Integration (Simple)");
    
    // Check if tmux is installed
    let output = Command::new("which")
        .arg("tmux")
        .output()?;
    
    if !output.status.success() {
        eprintln!("Error: tmux is not installed");
        return Err("tmux not installed".into());
    }
    println!("✓ Tmux is installed");
    
    // Create a test session
    let session_name = "ciab_test_session";
    
    // Check if session already exists
    let check = Command::new("tmux")
        .args(&["has-session", "-t", session_name])
        .output()?;
    
    if check.status.success() {
        println!("Session already exists, killing it first");
        Command::new("tmux")
            .args(&["kill-session", "-t", session_name])
            .output()?;
    }
    
    // Create new session
    let create = Command::new("tmux")
        .args(&[
            "new-session",
            "-d",  // Detached
            "-s", session_name,
            "-c", "/tmp",  // Working directory
            "/bin/bash",  // Command to run
        ])
        .output()?;
    
    if !create.status.success() {
        eprintln!("Failed to create session: {}", String::from_utf8_lossy(&create.stderr));
        return Err("Failed to create tmux session".into());
    }
    println!("✓ Created tmux session: {}", session_name);
    
    // List sessions
    let list = Command::new("tmux")
        .args(&["list-sessions"])
        .output()?;
    
    if list.status.success() {
        println!("✓ Current tmux sessions:");
        print!("{}", String::from_utf8_lossy(&list.stdout));
    }
    
    // Capture pane content
    let capture = Command::new("tmux")
        .args(&[
            "capture-pane",
            "-p",  // Print to stdout
            "-t", session_name,
        ])
        .output()?;
    
    if capture.status.success() {
        println!("✓ Captured pane content ({} bytes)", capture.stdout.len());
    }
    
    // Send a command to the session
    Command::new("tmux")
        .args(&[
            "send-keys",
            "-t", session_name,
            "echo 'Hello from tmux!'",
            "Enter",
        ])
        .output()?;
    
    // Wait a bit for command to execute
    std::thread::sleep(std::time::Duration::from_millis(100));
    
    // Capture again to see the output
    let capture2 = Command::new("tmux")
        .args(&[
            "capture-pane",
            "-p",
            "-t", session_name,
        ])
        .output()?;
    
    if capture2.status.success() {
        println!("✓ Session output:");
        println!("---");
        print!("{}", String::from_utf8_lossy(&capture2.stdout));
        println!("---");
    }
    
    // Clean up
    Command::new("tmux")
        .args(&["kill-session", "-t", session_name])
        .output()?;
    println!("✓ Killed tmux session");
    
    println!("\nAll tests passed!");
    Ok(())
}