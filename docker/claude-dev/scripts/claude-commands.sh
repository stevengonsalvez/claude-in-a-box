#!/bin/bash
# ABOUTME: Claude CLI convenience commands that output to Docker logs
# These commands make it easy to interact with Claude while capturing output

# Create convenient aliases for different Claude interaction modes
alias claude-print='/app/scripts/claude-logging.sh --print'
alias claude-script='/app/scripts/claude-logging.sh --script'
alias claude-start='/app/scripts/claude-logging.sh'

# Create functions for better user experience
claude-ask() {
    if [ $# -eq 0 ]; then
        echo "Usage: claude-ask \"your question here\""
        echo "Example: claude-ask \"How do I create a React component?\""
        return 1
    fi
    /app/scripts/claude-logging.sh --print "$*"
}

claude-help() {
    echo "🤖 Claude CLI Commands (with Docker log capture)"
    echo ""
    echo "Interactive modes:"
    echo "  claude-start          # Start interactive Claude CLI"
    echo "  claude                # Standard Claude CLI (responses not logged)"
    echo ""
    echo "Logged modes (output appears in Docker logs):"
    echo "  claude-ask \"question\" # Ask a single question with logged response"
    echo "  claude-print \"query\"  # Same as claude-ask"
    echo "  claude-script         # Read from stdin, useful for piping"
    echo ""
    echo "Examples:"
    echo "  claude-ask \"What files are in the current directory?\""
    echo "  echo \"Explain this code\" | claude-script"
    echo "  cat README.md | claude-script"
    echo ""
    echo "Note: Use logged modes to see Claude responses in the TUI logs!"
}

# Export functions so they're available in bash sessions
export -f claude-ask claude-help
