#!/bin/bash
# ABOUTME: Starts Claude CLI in proper interactive mode within tmux
# Handles the initial setup and ensures proper interactive session

echo "Starting Claude CLI in interactive mode..."

# Set up proper terminal environment for interactive use
export TERM=xterm-256color

# Ensure we're in a proper TTY context
export FORCE_COLOR=1
export CLAUDE_INTERACTIVE=1

# Configure Claude to avoid prompts
claude config set hasTrustDialogAccepted true >/dev/null 2>&1 || true

# Check if we have authentication
if [ -z "$ANTHROPIC_API_KEY" ] && [ ! -f ~/.claude.json ] && [ ! -f ~/.claude/.credentials.json ]; then
    echo "⚠️  No authentication found. Claude CLI needs authentication to work."
    echo "Please set ANTHROPIC_API_KEY or provide credentials."
    echo "Starting interactive shell instead..."
    echo "You can run 'claude' manually once authentication is set up."
    exec bash
fi

# Debug: Check what claude command we're about to run
echo "🔍 Debug: Running claude with environment:"
echo "   TERM=$TERM"
echo "   TTY: $(tty)"
echo "   Authentication method: $(if [ -n "$ANTHROPIC_API_KEY" ]; then echo "API Key"; elif [ -f ~/.claude.json ]; then echo "OAuth"; else echo "Credentials"; fi)"

# Start Claude CLI with proper stdin handling
echo "🚀 Launching Claude CLI in interactive mode..."
echo "You can now chat with Claude. Type your questions and press Enter."
echo "Use Ctrl-b then d to detach from this session."
echo "----------------------------------------"

# Use script to ensure proper TTY allocation
script -q -c "claude" /dev/null