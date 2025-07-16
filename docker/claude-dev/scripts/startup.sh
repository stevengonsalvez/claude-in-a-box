#!/bin/bash
# ABOUTME: Startup script for claude-dev container
# Handles environment setup, authentication, and CLI initialization

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log() {
    echo -e "${BLUE}[claude-box]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[claude-box]${NC} $1"
}

error() {
    echo -e "${RED}[claude-box]${NC} $1"
}

success() {
    echo -e "${GREEN}[claude-box]${NC} $1"
}

# Load environment variables from .env if it exists
if [ -f /app/.env ]; then
    log "Loading environment variables from .env"
    set -a
    source /app/.env
    set +a
fi

# Check if we're in claude-box mode
if [ "${CLAUDE_BOX_MODE}" = "true" ]; then
    log "Running in claude-box mode"
fi

# Check for existing authentication
AUTH_OK=false
if [ -f /home/claude-user/.claude.json ] && [ -s /home/claude-user/.claude.json ]; then
    log "Found existing Claude authentication"
    AUTH_OK=true
elif [ -n "${ANTHROPIC_API_KEY}" ]; then
    log "Found ANTHROPIC_API_KEY in environment"
    AUTH_OK=true
fi

if [ "${AUTH_OK}" = "false" ]; then
    warn "No Claude authentication found!"
    warn "Please ensure either:"
    warn "  1. ~/.claude.json is mounted to /home/claude-user/.claude.json"
    warn "  2. ANTHROPIC_API_KEY is set in environment"
fi

# Create .claude directory if it doesn't exist
mkdir -p /home/claude-user/.claude

# Copy CLAUDE.md template if it doesn't exist in workspace
if [ ! -f /workspace/CLAUDE.md ] && [ -f /app/config/CLAUDE.md.template ]; then
    log "Creating CLAUDE.md from template"
    cp /app/config/CLAUDE.md.template /workspace/CLAUDE.md
fi

# Determine which CLI to use
CLI_CMD="claude"
CLI_ARGS=("--no-git-ignore")

# Check if gemini mode is requested
if [ "${USE_GEMINI}" = "true" ] || [ "$1" = "gemini" ]; then
    CLI_CMD="gemini"
    CLI_ARGS=("--no-git-ignore")
    log "Using Gemini CLI"
else
    log "Using Claude CLI"
fi

# Add claude-box specific arguments
if [ "${CLAUDE_BOX_MODE}" = "true" ]; then
    CLI_ARGS+=("--claude-box")
fi

# If no command specified, run the CLI
if [ $# -eq 0 ] || [ "$1" = "claude" ] || [ "$1" = "gemini" ]; then
    success "Starting ${CLI_CMD} CLI..."
    exec "${CLI_CMD}" "${CLI_ARGS[@]}"
else
    # Run the specified command
    log "Running command: $*"
    exec "$@"
fi