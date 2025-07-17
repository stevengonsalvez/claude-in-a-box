// ABOUTME: Test UI display components including menu bar and help text

use claude_box::app::App;
use claude_box::components::LayoutComponent;
use ratatui::{backend::TestBackend, Terminal};

#[tokio::test]
async fn test_bottom_menu_bar_shows_refresh_key() {
    let mut app = App::new();
    app.state.load_mock_data();
    
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut layout = LayoutComponent::new();
    
    // Render the UI
    terminal.draw(|frame| {
        layout.render(frame, &app.state);
    }).unwrap();
    
    // Get the rendered buffer content
    let buffer = terminal.backend().buffer();
    let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();
    
    // Check that the bottom bar contains the refresh key
    assert!(content.contains("[f]refresh"), 
        "Bottom menu bar should contain '[f]refresh' but content was: {}", 
        content.chars().filter(|c| c.is_ascii_graphic() || *c == ' ').collect::<String>());
    
    // Also check for other expected menu items to ensure we're looking at the right place
    assert!(content.contains("[n]ew"), "Should contain '[n]ew'");
    assert!(content.contains("[?]help"), "Should contain '[?]help'");
    assert!(content.contains("[q]uit"), "Should contain '[q]uit'");
}

#[tokio::test] 
async fn test_help_screen_shows_refresh_key() {
    let mut app = App::new();
    app.state.load_mock_data();
    
    // Show help
    app.state.help_visible = true;
    
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut layout = LayoutComponent::new();
    
    // Render the UI with help visible
    terminal.draw(|frame| {
        layout.render(frame, &app.state);
    }).unwrap();
    
    // Get the rendered buffer content
    let buffer = terminal.backend().buffer();
    let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();
    
    // Check that help contains the refresh key
    assert!(content.contains("f          Refresh workspaces"), 
        "Help screen should contain 'f          Refresh workspaces' but content was: {}", 
        content.chars().filter(|c| c.is_ascii_graphic() || *c == ' ').collect::<String>());
    
    // Check that it's under Session Actions
    assert!(content.contains("Session Actions:"), "Should contain 'Session Actions:' section");
    
    // Verify other help items are present
    assert!(content.contains("Navigation:"), "Should contain 'Navigation:' section");
    assert!(content.contains("General:"), "Should contain 'General:' section");
}

#[tokio::test]
async fn test_refresh_key_in_help_under_session_actions() {
    let mut app = App::new();
    app.state.load_mock_data();
    app.state.help_visible = true;
    
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut layout = LayoutComponent::new();
    
    terminal.draw(|frame| {
        layout.render(frame, &app.state);
    }).unwrap();
    
    let buffer = terminal.backend().buffer();
    let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();
    
    // Find the position of "Session Actions:"
    let session_actions_pos = content.find("Session Actions:");
    assert!(session_actions_pos.is_some(), "Should find 'Session Actions:' section");
    
    // Find the position of "Views:" which comes after Session Actions
    let views_pos = content.find("Views:");
    assert!(views_pos.is_some(), "Should find 'Views:' section");
    
    // Find the refresh key entry
    let refresh_pos = content.find("f          Refresh workspaces");
    assert!(refresh_pos.is_some(), "Should find refresh key entry");
    
    // Verify that refresh key appears between Session Actions and Views
    let session_pos = session_actions_pos.unwrap();
    let view_pos = views_pos.unwrap();
    let ref_pos = refresh_pos.unwrap();
    
    assert!(ref_pos > session_pos && ref_pos < view_pos,
        "Refresh key should appear between Session Actions and Views sections. \
        Session Actions at {}, Refresh at {}, Views at {}",
        session_pos, ref_pos, view_pos);
}