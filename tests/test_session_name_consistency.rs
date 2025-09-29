// ABOUTME: Test for session name consistency between creation and attachment
// Verifies that tmux session names are consistent across all parts of the system

use claude_box::models::Session;

#[test]
fn test_session_name_generation_consistency() {
    // GREEN: Test that all session name generation uses the same logic consistently

    let branch_name = "feature/test-name-generation";

    // Test Session::new name generation
    let session_model = Session::new(
        branch_name.to_string(),
        "/tmp/test".to_string()
    );

    // All special characters should be replaced with underscores consistently
    let expected_name = format!("ciab_{}", Session::sanitize_tmux_name(branch_name));

    assert_eq!(
        session_model.tmux_session_name,
        expected_name,
        "Session model name generation should handle all special characters consistently"
    );

    // Verify that the expected name has all special characters replaced
    assert_eq!(expected_name, "ciab_feature_test-name-generation");
    assert!(!expected_name.contains('/'), "Forward slashes should be replaced");
}

#[test]
fn test_sanitize_tmux_name_comprehensive() {
    // Test the centralized sanitization function handles all problematic characters

    let problematic_name = "feat/branch:with<many>\"chars\"&(test)";
    let sanitized = Session::sanitize_tmux_name(problematic_name);

    // All special characters should be replaced with underscores
    let expected = "feat_branch_with_many__chars___test_";
    assert_eq!(sanitized, expected);

    // Verify no problematic characters remain
    let problematic_chars = ['/', '\\', ':', ';', '|', '&', '(', ')', '<', '>', '"', '\''];
    for ch in problematic_chars {
        assert!(!sanitized.contains(ch), "Character '{}' should be replaced", ch);
    }
}

#[test]
fn test_session_name_consistency_across_modules() {
    // Test that Session model and TmuxSession would generate the same names

    let test_name = "feature/complex:branch-name";

    // Session model approach
    let session = Session::new(test_name.to_string(), "/tmp/test".to_string());

    // TmuxSession approach (simulated)
    let tmux_name = format!("ciab_{}", Session::sanitize_tmux_name(test_name));

    assert_eq!(session.tmux_session_name, tmux_name,
               "Session model and TmuxSession should generate identical names");
}