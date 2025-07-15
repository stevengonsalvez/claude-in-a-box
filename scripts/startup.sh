#!/bin/bash
# ABOUTME: Startup script for claude-docker container with MCP server
# ABOUTME: Loads twilio env vars, checks for .credentials.json, copies CLAUDE.md template if no claude.md in claude-docker/claude-home.
# ABOUTME: Starts claude code or gemini cli with permissions bypass and continues from last session.

# Load environment variables from .env if it exists
# Use the .env file baked into the image at build time
if [ -f /app/.env ]; then
    echo "Loading environment from baked-in .env file"
    set -a
    source /app/.env 2>/dev/null || true
    set +a
    
    # Export Twilio variables for runtime use
    export TWILIO_ACCOUNT_SID
    export TWILIO_AUTH_TOKEN
    export TWILIO_FROM_NUMBER
    export TWILIO_TO_NUMBER
else
    echo "WARNING: No .env file found in image."
fi

# Check for existing authentication
if [ -f "$HOME/.claude/.credentials.json" ]; then
    echo "Found existing Claude authentication"
else
    echo "No existing authentication found - you will need to log in"
    echo "Your login will be saved for future sessions"
fi

# Handle CLAUDE.md template
if [ ! -f "$HOME/.claude/CLAUDE.md" ]; then
    echo "✓ No CLAUDE.md found at $HOME/.claude/CLAUDE.md - copying template"
    # Copy from the template that was baked into the image
    if [ -f "/app/.claude/CLAUDE.md" ]; then
        cp "/app/.claude/CLAUDE.md" "$HOME/.claude/CLAUDE.md"
    elif [ -f "/home/claude-user/.claude.template/CLAUDE.md" ]; then
        # Fallback for existing images
        cp "/home/claude-user/.claude.template/CLAUDE.md" "$HOME/.claude/CLAUDE.md"
    fi
    echo "  Template copied to: $HOME/.claude/CLAUDE.md"
else
    echo "✓ Using existing CLAUDE.md from $HOME/.claude/CLAUDE.md"
    echo "  This maps to: ~/.claude-docker/claude-home/CLAUDE.md on your host"
    echo "  To reset to template, delete this file and restart"
fi

# Verify Twilio MCP configuration
if [ -n "$TWILIO_ACCOUNT_SID" ] && [ -n "$TWILIO_AUTH_TOKEN" ]; then
    echo "✓ Twilio MCP server configured - SMS notifications enabled"
else
    echo "No Twilio credentials found - SMS notifications disabled"
fi

# Determine which AI CLI to start based on arguments or environment
AI_CLI="claude"  # Default to Claude
AI_ARGS="$CLAUDE_CONTINUE_FLAG --dangerously-skip-permissions"

# Check if user wants to use Gemini CLI
if [[ "$1" == "--gemini" ]] || [[ "$AI_CLI_PREFERENCE" == "gemini" ]]; then
    AI_CLI="gemini"
    # Remove --gemini flag from arguments if present
    if [[ "$1" == "--gemini" ]]; then
        shift
    fi
    # Set Gemini-specific arguments
    AI_ARGS="--yolo"  # Equivalent to --dangerously-skip-permissions
    echo "Starting Gemini CLI..."
    
    # Check if GOOGLE_AI_STUDIO_API_KEY is set
    if [ -z "$GOOGLE_AI_STUDIO_API_KEY" ]; then
        echo "WARNING: GOOGLE_AI_STUDIO_API_KEY not set in .env file"
        echo "You may need to authenticate or set the API key"
    fi
else
    echo "Starting Claude Code..."
fi

# Start the selected AI CLI
exec $AI_CLI $AI_ARGS "$@"
