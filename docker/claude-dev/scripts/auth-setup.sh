#!/bin/bash
# ABOUTME: Authentication setup script for claude-in-a-box
# Runs OAuth login and stores credentials for container sessions

set -e

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

log "No valid credentials found. Starting OAuth login process..."
log ""
log "This will open your browser to authenticate with Claude."
log "After authentication, credentials will be stored for all claude-box sessions."
log ""

# Run Claude login
if claude auth login; then
    success "Authentication successful!"
    
    # Verify credentials were created
    if [ -f /home/claude-user/.claude/.credentials.json ] && [ -s /home/claude-user/.claude/.credentials.json ]; then
        success "Credentials saved to ~/.claude-in-a-box/auth/.credentials.json"
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
    exit 1
fi