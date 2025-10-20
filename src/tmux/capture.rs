// ABOUTME: Tmux pane content capture utilities
//
// Provides functions for capturing tmux pane content with various options,
// including full scrollback history, visible pane content, and ANSI escape
// sequence preservation.

use anyhow::Result;
use tokio::process::Command;

/// Options for capturing tmux pane content
#[derive(Debug, Clone)]
pub struct CaptureOptions {
    /// Start line for capture ("-" for start of history, None for current visible area)
    pub start_line: Option<String>,
    /// End line for capture ("-" for end of history, None for current visible area)
    pub end_line: Option<String>,
    /// Whether to include ANSI escape sequences in the output
    pub include_escape_sequences: bool,
    /// Whether to join wrapped lines
    pub join_wrapped_lines: bool,
}

impl Default for CaptureOptions {
    fn default() -> Self {
        Self {
            start_line: None,
            end_line: None,
            include_escape_sequences: true,
            join_wrapped_lines: true,
        }
    }
}

impl CaptureOptions {
    /// Create options for capturing visible pane content only
    pub fn visible() -> Self {
        Self::default()
    }

    /// Create options for capturing full scrollback history
    pub fn full_history() -> Self {
        Self {
            start_line: Some("-".to_string()),
            end_line: Some("-".to_string()),
            include_escape_sequences: true,
            join_wrapped_lines: true,
        }
    }
}

/// Capture content from a tmux pane
///
/// # Arguments
/// * `session_name` - The name of the tmux session
/// * `options` - Capture options specifying what and how to capture
///
/// # Returns
/// * `Result<String>` - The captured content or an error
pub async fn capture_pane(session_name: &str, options: CaptureOptions) -> Result<String> {
    let mut args = vec!["capture-pane", "-p", "-t", session_name];

    // Add escape sequence flag if requested
    if options.include_escape_sequences {
        args.push("-e");
    }

    // Add join wrapped lines flag if requested
    if options.join_wrapped_lines {
        args.push("-J");
    }

    // Add start line if specified
    let mut start_arg = String::new();
    if let Some(start) = &options.start_line {
        start_arg = format!("-S{}", start);
        args.push(&start_arg);
    }

    // Add end line if specified
    let mut end_arg = String::new();
    if let Some(end) = &options.end_line {
        end_arg = format!("-E{}", end);
        args.push(&end_arg);
    }

    let output = Command::new("tmux").args(&args).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to capture pane content: {}", stderr);
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_options_default() {
        let opts = CaptureOptions::default();
        assert!(opts.include_escape_sequences);
        assert!(opts.join_wrapped_lines);
        assert!(opts.start_line.is_none());
        assert!(opts.end_line.is_none());
    }

    #[test]
    fn test_capture_options_visible() {
        let opts = CaptureOptions::visible();
        assert!(opts.include_escape_sequences);
        assert!(opts.join_wrapped_lines);
        assert!(opts.start_line.is_none());
        assert!(opts.end_line.is_none());
    }

    #[test]
    fn test_capture_options_full_history() {
        let opts = CaptureOptions::full_history();
        assert!(opts.include_escape_sequences);
        assert!(opts.join_wrapped_lines);
        assert_eq!(opts.start_line, Some("-".to_string()));
        assert_eq!(opts.end_line, Some("-".to_string()));
    }
}
