// ABOUTME: Unit tests for event handling to ensure keyboard inputs map to correct app actions

use claude_box::app::{AppState, EventHandler};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn create_key_event(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn create_key_event_with_modifiers(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

#[test]
fn test_quit_key_events() {
    let mut state = AppState::default();
    
    let quit_event1 = EventHandler::handle_key_event(create_key_event(KeyCode::Char('q')), &mut state);
    assert!(quit_event1.is_some());
    
    let quit_event2 = EventHandler::handle_key_event(create_key_event(KeyCode::Esc), &mut state);
    assert!(quit_event2.is_some());
    
    let quit_event3 = EventHandler::handle_key_event(
        create_key_event_with_modifiers(KeyCode::Char('c'), KeyModifiers::CONTROL), 
        &mut state
    );
    assert!(quit_event3.is_some());
}

#[test]
fn test_navigation_key_events() {
    let mut state = AppState::default();
    
    let down_event = EventHandler::handle_key_event(create_key_event(KeyCode::Char('j')), &mut state);
    assert!(down_event.is_some());
    
    let up_event = EventHandler::handle_key_event(create_key_event(KeyCode::Char('k')), &mut state);
    assert!(up_event.is_some());
    
    let left_event = EventHandler::handle_key_event(create_key_event(KeyCode::Char('h')), &mut state);
    assert!(left_event.is_some());
    
    let right_event = EventHandler::handle_key_event(create_key_event(KeyCode::Char('l')), &mut state);
    assert!(right_event.is_some());
}

#[test]
fn test_arrow_key_navigation() {
    let mut state = AppState::default();
    
    let down_arrow = EventHandler::handle_key_event(create_key_event(KeyCode::Down), &mut state);
    assert!(down_arrow.is_some());
    
    let up_arrow = EventHandler::handle_key_event(create_key_event(KeyCode::Up), &mut state);
    assert!(up_arrow.is_some());
    
    let left_arrow = EventHandler::handle_key_event(create_key_event(KeyCode::Left), &mut state);
    assert!(left_arrow.is_some());
    
    let right_arrow = EventHandler::handle_key_event(create_key_event(KeyCode::Right), &mut state);
    assert!(right_arrow.is_some());
}

#[test]
fn test_action_key_events() {
    let mut state = AppState::default();
    
    let new_event = EventHandler::handle_key_event(create_key_event(KeyCode::Char('n')), &mut state);
    assert!(new_event.is_some());
    
    let attach_event = EventHandler::handle_key_event(create_key_event(KeyCode::Char('a')), &mut state);
    assert!(attach_event.is_some());
    
    let start_stop_event = EventHandler::handle_key_event(create_key_event(KeyCode::Char('s')), &mut state);
    assert!(start_stop_event.is_some());
    
    let delete_event = EventHandler::handle_key_event(create_key_event(KeyCode::Char('d')), &mut state);
    assert!(delete_event.is_some());
}

#[test]
fn test_help_key_event() {
    let mut state = AppState::default();
    
    let help_event = EventHandler::handle_key_event(create_key_event(KeyCode::Char('?')), &mut state);
    assert!(help_event.is_some());
}

#[test]
fn test_help_visible_only_responds_to_help_and_esc() {
    let mut state = AppState::default();
    state.help_visible = true;
    
    let help_event = EventHandler::handle_key_event(create_key_event(KeyCode::Char('?')), &mut state);
    assert!(help_event.is_some());
    
    let esc_event = EventHandler::handle_key_event(create_key_event(KeyCode::Esc), &mut state);
    assert!(esc_event.is_some());
    
    let other_event = EventHandler::handle_key_event(create_key_event(KeyCode::Char('j')), &mut state);
    assert!(other_event.is_none());
}

#[test]
fn test_go_to_top_bottom() {
    let mut state = AppState::default();
    
    let go_top = EventHandler::handle_key_event(create_key_event(KeyCode::Char('g')), &mut state);
    assert!(go_top.is_some());
    
    let go_bottom = EventHandler::handle_key_event(create_key_event(KeyCode::Char('G')), &mut state);
    assert!(go_bottom.is_some());
}

#[test]
fn test_unknown_key_returns_none() {
    let mut state = AppState::default();
    
    let unknown_event = EventHandler::handle_key_event(create_key_event(KeyCode::Char('x')), &mut state);
    assert!(unknown_event.is_none());
    
    let unknown_f_key = EventHandler::handle_key_event(create_key_event(KeyCode::F(1)), &mut state);
    assert!(unknown_f_key.is_none());
}

#[test]
fn test_process_quit_event() {
    let mut state = AppState::default();
    
    assert!(!state.should_quit);
    
    if let Some(event) = EventHandler::handle_key_event(create_key_event(KeyCode::Char('q')), &mut state) {
        EventHandler::process_event(event, &mut state);
    }
    
    assert!(state.should_quit);
}

#[test]
fn test_process_help_toggle_event() {
    let mut state = AppState::default();
    
    assert!(!state.help_visible);
    
    if let Some(event) = EventHandler::handle_key_event(create_key_event(KeyCode::Char('?')), &mut state) {
        EventHandler::process_event(event, &mut state);
    }
    
    assert!(state.help_visible);
}