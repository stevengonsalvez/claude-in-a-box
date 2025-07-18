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

# Check for existing authentication (multiple sources)
AUTH_OK=false
AUTH_SOURCES=()

# Check for .claude.json at root level (primary location for Claude CLI)
if [ -f /home/claude-user/.claude.json ] && [ -s /home/claude-user/.claude.json ]; then
    AUTH_SOURCES+=(".claude.json (mounted)")
    AUTH_OK=true
fi

# Check for .claude directory with credentials
if [ -f /home/claude-user/.claude/.credentials.json ] && [ -s /home/claude-user/.claude/.credentials.json ]; then
    AUTH_SOURCES+=(".claude/.credentials.json")
    AUTH_OK=true
fi

# Check for environment variable
if [ -n "${ANTHROPIC_API_KEY}" ]; then
    AUTH_SOURCES+=("ANTHROPIC_API_KEY environment variable")
    AUTH_OK=true
fi

if [ "${AUTH_OK}" = "true" ]; then
    log "Found Claude authentication via: ${AUTH_SOURCES[*]}"
else
    warn "No Claude authentication found!"
    warn "Please ensure one of:"
    warn "  1. Have ~/.claude.json on host (mounted to /home/claude-user/.claude.json)"
    warn "  2. Have ~/.claude/.credentials.json on host (mounted to /home/claude-user/.claude/.credentials.json)"
    warn "  3. Set ANTHROPIC_API_KEY in environment"
fi

# Create .claude directory if it doesn't exist
mkdir -p /home/claude-user/.claude

# Configure GitHub CLI if GITHUB_TOKEN is provided
if [ -n "${GITHUB_TOKEN}" ]; then
    log "Configuring GitHub CLI with token authentication"
    echo "${GITHUB_TOKEN}" | gh auth login --with-token
    
    # Configure git to use the token for authentication
    git config --global credential.helper store
    echo "https://oauth:${GITHUB_TOKEN}@github.com" > /home/claude-user/.git-credentials
    
    # Test gh CLI connection
    if gh auth status > /dev/null 2>&1; then
        success "GitHub CLI authenticated successfully"
        log "Available commands: gh issue list, gh pr list, gh repo view, etc."
    else
        warn "GitHub CLI authentication failed"
    fi
else
    warn "GITHUB_TOKEN not found - GitHub CLI and token-based git auth unavailable"
    log "SSH keys will be used for git operations if available"
fi

# Copy CLAUDE.md template if it doesn't exist in workspace
if [ ! -f /workspace/CLAUDE.md ] && [ -f /app/config/CLAUDE.md.template ]; then
    log "Creating CLAUDE.md from template"
    cp /app/config/CLAUDE.md.template /workspace/CLAUDE.md
fi

# Ensure theme preferences are set to avoid theme prompt
# Check if .claude.json exists and has theme settings
if [ -f /home/claude-user/.claude.json ]; then
    # Check if theme-command counter exists, if not add it
    if ! grep -q '"theme-command"' /home/claude-user/.claude.json 2>/dev/null; then
        log "Adding theme preferences to avoid theme prompt"
        # Create a temporary file with theme preferences added
        python3 -c "
import json
try:
    with open('/home/claude-user/.claude.json', 'r') as f:
        data = json.load(f)
    data['theme-command'] = 10  # Set high enough to avoid theme prompt
    with open('/home/claude-user/.claude.json', 'w') as f:
        json.dump(data, f, indent=2)
except:
    pass
" 2>/dev/null || true
    fi
fi

# Determine which CLI to use (adapted from claude-docker startup.sh)
CLI_CMD="claude"
CLI_ARGS="$CLAUDE_CONTINUE_FLAG --dangerously-skip-permissions"

log "Using Claude CLI with args: $CLI_ARGS"

# If no command specified, run the CLI in interactive mode
if [ $# -eq 0 ] || [ "$1" = "claude" ]; then
    success "Starting ${CLI_CMD} CLI in interactive mode..."
    success "Container is ready! You can attach to it to interact with Claude CLI."
    success "Use: docker exec -it <container-name> bash"
    
    # Keep the container running by sleeping indefinitely
    # Users can attach to the container and run claude commands interactively
    log "Container will stay running. Attach to interact with Claude CLI."
    exec sleep infinity
else
    # Run the specified command
    log "Running command: $*"
    exec "$@"
fi