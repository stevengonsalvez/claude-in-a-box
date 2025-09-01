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

# Check claude-box session mode
if [ "${CLAUDE_BOX_MODE}" = "boss" ]; then
    log "Running in claude-box boss mode"
elif [ "${CLAUDE_BOX_MODE}" = "interactive" ]; then
    log "Running in claude-box interactive mode"
elif [ "${CLAUDE_BOX_MODE}" = "true" ]; then
    # Legacy support
    log "Running in claude-box mode (legacy)"
fi

# Check for existing authentication (multiple sources)
AUTH_OK=false
AUTH_SOURCES=()

# Handle authentication for parallel sessions
# Priority: 1. Mounted .claude.json (OAuth tokens), 2. Environment variable, 3. Credentials file

# Check for mounted .claude.json first (OAuth tokens from Claude Max)
if [ -f /home/claude-user/.claude.json ] && [ -s /home/claude-user/.claude.json ]; then
    AUTH_SOURCES+=(".claude.json (OAuth tokens)")
    AUTH_OK=true
    log "Using mounted .claude.json with OAuth authentication"
elif [ -n "${ANTHROPIC_API_KEY}" ]; then
    AUTH_SOURCES+=("ANTHROPIC_API_KEY environment variable")
    AUTH_OK=true
    log "Using ANTHROPIC_API_KEY environment variable for authentication"
fi

# Check for .claude directory with credentials (if no auth found yet)
if [ "${AUTH_OK}" = "false" ] && [ -f /home/claude-user/.claude/.credentials.json ] && [ -s /home/claude-user/.claude/.credentials.json ]; then
    AUTH_SOURCES+=(".claude/.credentials.json (claude-in-a-box)")
    AUTH_OK=true
fi

if [ "${AUTH_OK}" = "true" ]; then
    log "Found Claude authentication via: ${AUTH_SOURCES[*]}"
else
    warn "No Claude authentication found!"
    warn "Please ensure one of:"
    warn "  1. Run 'claude-box auth' to set up authentication"
    warn "  2. Have ~/.claude-in-a-box/auth/.credentials.json (mounted to /home/claude-user/.claude/.credentials.json)"
    warn "  3. Set ANTHROPIC_API_KEY in environment"
fi

# Create .claude directory if it doesn't exist (unless it's already mounted)
if [ ! -d /home/claude-user/.claude ]; then
    mkdir -p /home/claude-user/.claude
fi

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

# Set up Claude CLI logging commands
log "Setting up Claude CLI logging commands"
if [ -f /app/scripts/claude-commands.sh ]; then
    source /app/scripts/claude-commands.sh
    log "✅ Claude logging commands available: claude-ask, claude-print, claude-script"
else
    warn "Claude logging commands not found"
fi

# Configure Claude CLI settings for permissions
log "Configuring Claude CLI settings"

# Check if .claude is a mounted directory (from host)
if mountpoint -q /home/claude-user/.claude 2>/dev/null; then
    log ".claude directory is mounted from host"
    # If mounted and no settings.json exists, copy our default there
    if [ ! -f /home/claude-user/.claude/settings.json ]; then
        if [ -f /app/config/claude-settings.json ]; then
            log "Copying default settings to mounted .claude directory"
            cp /app/config/claude-settings.json /home/claude-user/.claude/settings.json
            success "Claude settings configured with pre-approved tools"
        else
            warn "Default Claude settings template not found"
        fi
    else
        log "Existing Claude settings found in mounted directory, preserving"
    fi
else
    # Not mounted, handle normally
    if [ ! -f /home/claude-user/.claude/settings.json ]; then
        if [ -f /app/config/claude-settings.json ]; then
            log "Applying default Claude settings with pre-approved permissions"
            mkdir -p /home/claude-user/.claude
            cp /app/config/claude-settings.json /home/claude-user/.claude/settings.json
            chown -R claude-user:claude-user /home/claude-user/.claude
            success "Claude settings configured with pre-approved tools"
        else
            warn "Default Claude settings template not found"
        fi
    else
        log "Existing Claude settings found, preserving user configuration"
    fi
fi

# Ensure theme preferences are set to avoid Claude CLI theme prompt
# Check if theme is already configured
if ! claude config get -g theme >/dev/null 2>&1; then
    log "Setting default theme to avoid theme selection prompt"
    claude config set -g theme dark
else
    log "Theme already configured: $(claude config get -g theme 2>/dev/null || echo 'unknown')"
fi

# Trust dialog should be accepted via the settings.json
# Log the status for debugging
if [ -f /home/claude-user/.claude/settings.json ]; then
    log "Claude settings file exists, trust dialog should be configured"
else
    # If no settings file exists, set trust dialog directly
    log "No settings file found, setting trust dialog acceptance directly"
    /home/claude-user/.npm-global/bin/claude config set hasTrustDialogAccepted true >/dev/null 2>&1 || warn "Failed to set trust dialog config"
fi

# Determine which CLI to use (adapted from claude-docker startup.sh)
# Use the direct binary path to avoid wrapper functions
CLI_CMD="/home/claude-user/.npm-global/bin/claude"
CLI_ARGS="$CLAUDE_CONTINUE_FLAG"

# Only log CLI args if they're actually being used (boss mode)
if [ "${CLAUDE_BOX_MODE}" = "boss" ] && [ -n "$CLI_ARGS" ]; then
    log "Boss mode will use additional Claude CLI args: $CLI_ARGS"
fi

# Handle boss mode execution
if [ "${CLAUDE_BOX_MODE}" = "boss" ] && [ -n "${CLAUDE_BOX_PROMPT}" ]; then
    # Create log directory
    mkdir -p /workspace/.claude-box/logs

    success "Container environment ready!"
    if [ "${AUTH_OK}" = "true" ]; then
        success "✅ Authentication detected - Claude will work immediately"
        log "🤖 Executing boss mode prompt..."
        log "Prompt: ${CLAUDE_BOX_PROMPT}"

        # Boss mode prompt text to append
        BOSS_MODE_PROMPT="Ultrathink and understand our project rules, particularly around testing. You must go test first, and you must work in a way that allows for small known-good increments. You must commit when the code is in a working state, and commit early and often. When committing: - Use conventional commit format (feat:, fix:, refactor:, test:, docs:) - Commit after each logical increment (test passes, feature complete, refactor done) - Generate descriptive commit messages that explain the 'what' and 'why' - Never leave code in a broken state between commits"

        # Append boss mode prompt to user prompt
        ENHANCED_PROMPT="${CLAUDE_BOX_PROMPT} ${BOSS_MODE_PROMPT}"

        # Execute Claude with the enhanced prompt and text output
        log "Running: claude --print --output-format text --verbose \"${ENHANCED_PROMPT}\""
        exec claude --print --output-format text --verbose "${ENHANCED_PROMPT}" $CLI_ARGS
    else
        error "❌ Boss mode requires authentication!"
        error "Please ensure one of:"
        error "  1. Run 'claude-box auth' to set up authentication"
        error "  2. Have ~/.claude-in-a-box/auth/.credentials.json mounted"
        error "  3. Set ANTHROPIC_API_KEY in environment"
        exit 1
    fi
elif [ "${CLAUDE_BOX_MODE}" = "boss" ]; then
    error "❌ Boss mode requires a prompt!"
    error "CLAUDE_BOX_PROMPT environment variable is missing or empty"
    exit 1
fi

# Create log directory (if not exists)
mkdir -p /workspace/.claude-box/logs

# Debug: Log script arguments
log "DEBUG: Script called with $# arguments"
log "DEBUG: Arguments: $@"

# If no command specified, run Claude CLI as main process
if [ $# -eq 0 ]; then
    success "Container environment ready!"
    if [ "${AUTH_OK}" = "true" ]; then
        success "✅ Authentication detected - Claude CLI starting"
        log "Starting Claude CLI as main process in interactive mode"
        # Debug: Log what we're about to execute
        log "DEBUG: CLI_CMD='$CLI_CMD'"
        log "DEBUG: CLI_ARGS='$CLI_ARGS'"
        log "DEBUG: CLAUDE_CONTINUE_FLAG='$CLAUDE_CONTINUE_FLAG'"
        # Debug: Check for any Claude-related environment variables
        log "DEBUG: Environment variables containing CLAUDE:"
        env | grep -i claude | while read line; do
            log "DEBUG: $line"
        done
        # Debug: Check TTY status
        if [ -t 0 ]; then
            log "DEBUG: stdin is a TTY"
        else
            log "DEBUG: stdin is NOT a TTY"
        fi
        if [ -t 1 ]; then
            log "DEBUG: stdout is a TTY"
        else
            log "DEBUG: stdout is NOT a TTY"
        fi
        # Run Claude CLI directly as PID 1 in interactive mode
        # This allows Docker attach to connect directly to Claude
        # NOTE: Don't pass --dangerously-skip-permissions here as it causes issues with PID 1
        
        # Set the trust dialog as accepted directly via claude config
        log "Setting trust dialog acceptance via CLI config"
        $CLI_CMD config set hasTrustDialogAccepted true 2>/dev/null || true
        
        # Use the wrapper script to handle initial prompts automatically
        log "Starting Claude CLI with automatic prompt handling"
        exec /app/scripts/claude-wrapper.sh
    else
        error "❌ Authentication required to run Claude CLI!"
        error "Please ensure one of:"
        error "  1. Run 'claude-box auth' to set up authentication"
        error "  2. Have ~/.claude-in-a-box/auth/.credentials.json mounted"
        error "  3. Set ANTHROPIC_API_KEY in environment"
        # Keep container alive but show error
        exec sleep infinity
    fi
else
    # Run the specified command
    log "Running command: $*"
    exec "$@"
fi
