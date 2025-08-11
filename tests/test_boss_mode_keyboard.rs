// ABOUTME: Tests for boss mode keyboard functionality including legend display and option key combinations

use claude_box::app::{AppState, EventHandler};
use claude_box::app::events::AppEvent;
use claude_box::app::state::{NewSessionState, NewSessionStep, View, TextEditor};
use claude_box::components::NewSessionComponent;
use claude_box::models::SessionMode;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

fn create_key_event(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn create_key_event_with_modifiers(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

fn create_new_session_state_in_prompt_step() -> AppState {
    let mut state = AppState::default();
    state.current_view = View::NewSession;
    state.new_session_state = Some(NewSessionState::new_for_boss_mode());

    // Set the state to prompt input step
    if let Some(ref mut session_state) = state.new_session_state {
        session_state.step = NewSessionStep::InputPrompt;
        session_state.mode = SessionMode::Boss;
    }

    state
}

#[test]
fn test_legend_display_shows_only_arrows() {
    let mut state = create_new_session_state_in_prompt_step();
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut component = NewSessionComponent::new();

    // Render the UI
    terminal
        .draw(|frame| {
            let area = frame.area();
            component.render(frame, area, &state);
        })
        .unwrap();

    // Get the rendered buffer content
    let buffer = terminal.backend().buffer();
    let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

    // The legend should show arrows only, not hjkl
    assert!(
        content.contains("↑/↓") || content.contains("arrows: Move cursor"),
        "Legend should mention arrows for cursor movement, content: {}",
        content
            .chars()
            .filter(|c| c.is_ascii_graphic() || c.is_whitespace())
            .collect::<String>()
    );

    // Should NOT contain hjkl in the legend
    assert!(
        !content.contains("hjkl") && !content.contains("h/j/k/l"),
        "Legend should not contain hjkl references, content: {}",
        content
            .chars()
            .filter(|c| c.is_ascii_graphic() || c.is_whitespace())
            .collect::<String>()
    );
}

#[test]
fn test_hjkl_letters_are_treated_as_regular_characters() {
    let mut state = create_new_session_state_in_prompt_step();

    // Test that h, j, k, l are treated as regular characters in boss mode prompt
    let h_event = EventHandler::handle_key_event(
        create_key_event(KeyCode::Char('h')),
        &mut state,
    );
    assert_eq!(h_event, Some(AppEvent::NewSessionInputPromptChar('h')));

    let j_event = EventHandler::handle_key_event(
        create_key_event(KeyCode::Char('j')),
        &mut state,
    );
    assert_eq!(j_event, Some(AppEvent::NewSessionInputPromptChar('j')));

    let k_event = EventHandler::handle_key_event(
        create_key_event(KeyCode::Char('k')),
        &mut state,
    );
    assert_eq!(k_event, Some(AppEvent::NewSessionInputPromptChar('k')));

    let l_event = EventHandler::handle_key_event(
        create_key_event(KeyCode::Char('l')),
        &mut state,
    );
    assert_eq!(l_event, Some(AppEvent::NewSessionInputPromptChar('l')));
}

#[test]
fn test_arrow_keys_move_cursor() {
    let mut state = create_new_session_state_in_prompt_step();

    // Arrow keys should trigger cursor movement events
    let left_event = EventHandler::handle_key_event(
        create_key_event(KeyCode::Left),
        &mut state,
    );
    assert_eq!(left_event, Some(AppEvent::NewSessionCursorLeft));

    let right_event = EventHandler::handle_key_event(
        create_key_event(KeyCode::Right),
        &mut state,
    );
    assert_eq!(right_event, Some(AppEvent::NewSessionCursorRight));

    let up_event = EventHandler::handle_key_event(
        create_key_event(KeyCode::Up),
        &mut state,
    );
    assert_eq!(up_event, Some(AppEvent::NewSessionCursorUp));

    let down_event = EventHandler::handle_key_event(
        create_key_event(KeyCode::Down),
        &mut state,
    );
    assert_eq!(down_event, Some(AppEvent::NewSessionCursorDown));
}

#[test]
fn test_option_arrow_word_movement() {
    let mut state = create_new_session_state_in_prompt_step();

    // Option + left arrow should trigger word movement left
    let option_left_event = EventHandler::handle_key_event(
        create_key_event_with_modifiers(KeyCode::Left, KeyModifiers::ALT),
        &mut state,
    );
    assert_eq!(option_left_event, Some(AppEvent::NewSessionCursorWordLeft));

    // Option + right arrow should trigger word movement right
    let option_right_event = EventHandler::handle_key_event(
        create_key_event_with_modifiers(KeyCode::Right, KeyModifiers::ALT),
        &mut state,
    );
    assert_eq!(option_right_event, Some(AppEvent::NewSessionCursorWordRight));
}

#[test]
fn test_option_delete_word_deletion() {
    let mut state = create_new_session_state_in_prompt_step();

    // Option + delete should trigger word deletion
    let option_delete_event = EventHandler::handle_key_event(
        create_key_event_with_modifiers(KeyCode::Delete, KeyModifiers::ALT),
        &mut state,
    );
    assert_eq!(option_delete_event, Some(AppEvent::NewSessionDeleteWordForward));

    // Option + backspace should trigger word deletion backward
    let option_backspace_event = EventHandler::handle_key_event(
        create_key_event_with_modifiers(KeyCode::Backspace, KeyModifiers::ALT),
        &mut state,
    );
    assert_eq!(option_backspace_event, Some(AppEvent::NewSessionDeleteWordBackward));
}

#[test]
fn test_text_editor_word_boundaries() {
    // Test helper to identify word boundaries for word-wise operations
    let text = "hello world test_function some-text";
    let words = text.split_whitespace().collect::<Vec<_>>();
    assert_eq!(words, vec!["hello", "world", "test_function", "some-text"]);

    // Word boundaries should be whitespace and certain punctuation
    let test_cases = vec![
        ("hello world", vec![(0, 5), (6, 11)]),  // word boundaries
        ("test_function", vec![(0, 13)]),         // underscore is part of word
        ("some-text", vec![(0, 9)]),             // hyphen is part of word
        ("a.b c", vec![(0, 1), (2, 3), (4, 5)]), // dots separate words
    ];

    for (input, expected_word_bounds) in test_cases {
        let words: Vec<&str> = input.split_whitespace().collect();
        let mut start = 0;
        let mut actual_bounds = Vec::new();

        for word in &words {
            if let Some(pos) = input[start..].find(word) {
                let word_start = start + pos;
                let word_end = word_start + word.len();
                actual_bounds.push((word_start, word_end));
                start = word_end;
            }
        }

        println!("Input: '{}', Expected: {:?}, Got: {:?}", input, expected_word_bounds, actual_bounds);
    }
}

mod text_editor_word_navigation {
    use super::*;
    use claude_box::app::state::TextEditor;

    #[test]
    fn test_word_forward() {
        let mut editor = TextEditor::from_string("hello world test");
        assert_eq!(editor.get_cursor_position(), (0, 0));

        // Moving word forward should go to start of next word
        editor.move_cursor_word_forward();
        assert_eq!(editor.get_cursor_position(), (0, 6)); // "world"

        editor.move_cursor_word_forward();
        assert_eq!(editor.get_cursor_position(), (0, 12)); // "test"

        editor.move_cursor_word_forward();
        assert_eq!(editor.get_cursor_position(), (0, 16)); // end of line
    }

    #[test]
    fn test_word_backward() {
        let mut editor = TextEditor::from_string("hello world test");
        // Start at end
        editor.move_cursor_to_end();
        assert_eq!(editor.get_cursor_position(), (0, 16));

        // Moving word backward should go to start of current/previous word
        editor.move_cursor_word_backward();
        assert_eq!(editor.get_cursor_position(), (0, 12)); // "test"

        editor.move_cursor_word_backward();
        assert_eq!(editor.get_cursor_position(), (0, 6)); // "world"

        editor.move_cursor_word_backward();
        assert_eq!(editor.get_cursor_position(), (0, 0)); // "hello"
    }

    #[test]
    fn test_delete_word_forward() {
        let mut editor = TextEditor::from_string("hello world test");
        assert_eq!(editor.get_cursor_position(), (0, 0));

        // Delete word forward should delete "hello "
        editor.delete_word_forward();
        assert_eq!(editor.to_string(), "world test");
        assert_eq!(editor.get_cursor_position(), (0, 0));

        editor.delete_word_forward();
        assert_eq!(editor.to_string(), "test");
        assert_eq!(editor.get_cursor_position(), (0, 0));
    }

    #[test]
    fn test_delete_word_backward() {
        let mut editor = TextEditor::from_string("hello world test");
        // Move cursor to middle of "world"
        editor.set_cursor_position(0, 8);

        // Delete word backward should delete "world" up to cursor
        editor.delete_word_backward();
        assert_eq!(editor.to_string(), "hello rld test");

        // Move to after "hello"
        editor.set_cursor_position(0, 5);
        editor.delete_word_backward();
        assert_eq!(editor.to_string(), " rld test");
    }
}
