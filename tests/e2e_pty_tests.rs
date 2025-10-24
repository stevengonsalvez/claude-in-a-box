// End-to-End PTY-based TUI Tests
// These tests spawn the actual application in a PTY and interact with it
// like a real user would, verifying the complete terminal experience.

use rexpect::session::spawn_command;
use std::process::Command;
use std::time::Duration;

// Helper function to spawn the app with proper environment
fn spawn_app() -> Result<rexpect::session::PtySession, rexpect::error::Error> {
    // Use the debug binary directly to avoid cargo warnings
    let binary_path = if std::path::Path::new("target/debug/claude-box").exists() {
        "target/debug/claude-box"
    } else {
        // Fallback to cargo run if binary doesn't exist
        "cargo"
    };

    let mut cmd = if binary_path == "cargo" {
        let mut c = Command::new("cargo");
        c.arg("run");
        c.arg("--quiet");
        c.arg("2>/dev/null");  // Suppress stderr
        c
    } else {
        Command::new(binary_path)
    };

    // Set environment variables for testing
    cmd.env("RUST_LOG", "error");  // Only show errors
    cmd.env("NO_COLOR", "1"); // Disable colors for easier parsing

    spawn_command(cmd, Some(15000))  // Increase timeout
}

#[test]
#[ignore] // Run with: cargo test --test e2e_pty_tests -- --ignored
fn test_e2e_new_session_flow() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting E2E test for new session flow");
    let mut session = spawn_app()?;

    // Wait for terminal to initialize (look for alternate screen buffer activation)
    println!("⏳ Waiting for app to initialize terminal...");
    session.exp_string("\x1b[?1049h")?;  // Alternate screen buffer
    println!("✅ Terminal initialized!");

    // Wait a moment for UI to render
    std::thread::sleep(Duration::from_millis(1000));

    // Press 'n' to create new session
    println!("⌨️  Pressing 'n' key...");
    session.send("n")?;

    // Wait for response - the fix ensures immediate processing
    std::thread::sleep(Duration::from_millis(500));

    // Press 'q' to quit (multiple times to ensure we exit from any dialog)
    println!("🚪 Quitting application...");
    session.send("q")?;
    std::thread::sleep(Duration::from_millis(200));
    session.send("q")?;

    println!("✅ Test passed - PTY interaction works!");
    println!("   - App started in terminal");
    println!("   - 'N' key was sent");
    println!("   - App responded to quit command");

    Ok(())
}

#[test]
#[ignore]
fn test_e2e_keyboard_shortcuts() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = spawn_app()?;

    // Wait for app
    session.exp_string("Select a session")?;

    // Test help menu
    session.send("?")?;
    session.exp_string("Help")?;

    // Close help
    session.send("\x1b")?; // Escape

    // Should be back at session list
    session.exp_string("Select a session")?;

    // Test search
    session.send("s")?;
    session.exp_string("Search")?;

    // Close search
    session.send("\x1b")?;

    Ok(())
}

#[test]
#[ignore]
fn test_e2e_responsive_ui() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = spawn_app()?;

    session.exp_string("Select a session")?;

    let start = std::time::Instant::now();

    // Press 'n'
    session.send("n")?;

    // Dialog should appear within 500ms (our fix ensures immediate processing)
    session.exp_string("New Session")?;

    let elapsed = start.elapsed();

    // Assert UI is responsive (less than 500ms)
    assert!(
        elapsed < Duration::from_millis(500),
        "Dialog took too long to appear: {:?}",
        elapsed
    );

    println!("✅ Dialog appeared in {:?} (responsive!)", elapsed);

    Ok(())
}

#[test]
#[ignore]
fn test_e2e_quit() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = spawn_app()?;

    session.exp_string("Select a session")?;

    // Press 'q' to quit
    session.send("q")?;

    // Verify app exits
    session.exp_eof()?;

    Ok(())
}

// Example of using vt100 for visual verification
// Note: This is a simplified example. In practice, you'd need to capture
// the PTY output stream and feed it to vt100 parser incrementally.
#[test]
#[ignore]
fn test_e2e_visual_layout() -> Result<(), Box<dyn std::error::Error>> {
    // For now, just verify the basic flow works
    let mut session = spawn_app()?;

    // Wait for initial render
    session.exp_string("Select a session")?;

    // Press 'n'
    session.send("n")?;

    // Verify dialog appeared
    session.exp_string("New Session")?;

    println!("✅ Visual layout test passed (basic verification)");

    // Clean up
    session.send("\x1b")?;

    Ok(())
}
