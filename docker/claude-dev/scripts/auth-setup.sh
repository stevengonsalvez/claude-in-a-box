#!/bin/bash
# ABOUTME: Authentication setup script for claude-in-a-box
# Runs OAuth login and stores credentials for container sessions

set -e

# Check if running in non-interactive mode
NON_INTERACTIVE=${NON_INTERACTIVE:-false}

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log() {
    echo -e "${BLUE}[claude-box auth]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[claude-box auth]${NC} $1"
}

error() {
    echo -e "${RED}[claude-box auth]${NC} $1"
}

success() {
    echo -e "${GREEN}[claude-box auth]${NC} $1"
}

log "Starting Claude authentication setup for claude-in-a-box"

# Set up environment for claude-user
export PATH="/home/claude-user/.npm-global/bin:$PATH"
export HOME=/home/claude-user

# Check if claude command is available
if ! command -v claude >/dev/null 2>&1; then
    error "Claude CLI not found in PATH: $PATH"
    error "Available commands:"
    ls -la /home/claude-user/.npm-global/bin/
    exit 1
fi

log "Claude CLI found at: $(which claude)"
log "Claude CLI version: $(claude --version 2>&1 || echo 'version check failed')"

# Ensure the .claude directory exists
mkdir -p /home/claude-user/.claude

# Check if credentials already exist
if [ -f /home/claude-user/.claude/.credentials.json ] && [ -s /home/claude-user/.claude/.credentials.json ]; then
    log "Existing credentials found. Checking if they're valid..."
    
    # Test existing credentials
    if claude auth status >/dev/null 2>&1; then
        success "Existing credentials are valid!"
        success "Authentication setup complete - you can now use claude-box sessions"
        exit 0
    else
        warn "Existing credentials are invalid or expired. Setting up new authentication..."
        rm -f /home/claude-user/.claude/.credentials.json
    fi
fi

log "No valid credentials found. Starting authentication process..."

# Check which authentication method to use (OAuth by default)
AUTH_METHOD=${AUTH_METHOD:-oauth}

if [ "$AUTH_METHOD" = "oauth" ]; then
    log ""
    log "Starting OAuth authentication flow..."
    log "You'll be prompted to open a URL in your browser to complete authentication."
    log ""
    
    # Run Claude OAuth login command (interactive)
    claude auth login
    AUTH_SUCCESS=$?
elif [ "$AUTH_METHOD" = "token" ]; then
    log ""
    log "Starting API token authentication..."
    log "You'll be prompted to enter your Anthropic API token."
    log ""
    log "If you don't have an API token, get one from: https://console.anthropic.com/"
    log ""
    
    # Run Claude setup-token command (interactive)
    claude setup-token
    AUTH_SUCCESS=$?
else
    error "Unknown authentication method: $AUTH_METHOD"
    error "Supported methods: oauth, token"
    exit 1
fi

if [ $AUTH_SUCCESS -eq 0 ]; then
    success "Authentication successful!"
    
    # Give Claude CLI a moment to finish writing files
    sleep 2
    
    # Verify credentials were created
    if [ -f /home/claude-user/.claude/.credentials.json ] && [ -s /home/claude-user/.claude/.credentials.json ]; then
        success "Credentials saved to ~/.claude-in-a-box/auth/.credentials.json"
        
        # Always try to copy .claude.json if it exists (for theme preferences and OAuth tokens)
        if [ -f /home/claude-user/.claude.json ]; then
            if cp /home/claude-user/.claude.json /home/claude-user/.claude/.claude.json; then
                success "Configuration saved to ~/.claude-in-a-box/auth/.claude.json"
            else
                warn "Failed to copy .claude.json configuration file"
            fi
        else
            warn ".claude.json not found - Claude CLI configuration may not be available"
            log "Expected location: /home/claude-user/.claude.json"
            log "Note: Claude CLI currently ignores XDG Base Directory specification"
            log "Available files in home:"
            ls -la /home/claude-user/ | grep -E "\.(json|credentials)" || log "No config files found"
            
            # Future-proofing: check XDG locations too
            if [ -n "$XDG_CONFIG_HOME" ] && [ -f "$XDG_CONFIG_HOME/claude/config.json" ]; then
                log "Found XDG config at: $XDG_CONFIG_HOME/claude/config.json"
                cp "$XDG_CONFIG_HOME/claude/config.json" /home/claude-user/.claude/.claude.json
                success "Copied XDG configuration to auth directory"
            elif [ -f /home/claude-user/.config/claude/config.json ]; then
                log "Found XDG config at: ~/.config/claude/config.json"
                cp /home/claude-user/.config/claude/config.json /home/claude-user/.claude/.claude.json
                success "Copied XDG configuration to auth directory"
            fi
        fi
        
        success ""
        success "🎉 Authentication setup complete!"
        success "You can now use claude-box sessions with these credentials."
        success ""
        success "To start a development session, run:"
        success "  claude-box session start"
    else
        error "Authentication succeeded but credentials file not found!"
        error "This may indicate an issue with the authentication process."
        exit 1
    fi
else
    error "Authentication failed!"
    error "Please try running the auth setup again."
    if [ "$NON_INTERACTIVE" = "true" ]; then
        error "Make sure you completed the OAuth flow in your browser."
    fi
    exit 1
fi