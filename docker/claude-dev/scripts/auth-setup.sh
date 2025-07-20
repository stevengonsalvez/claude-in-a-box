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

if [ "$NON_INTERACTIVE" = "true" ]; then
    log ""
    log "Running in non-interactive mode."
    log "The OAuth URL will be displayed below for you to open manually."
    log ""
    
    # Run Claude login with --no-open flag to prevent browser auto-open
    # Capture the output to extract the OAuth URL
    AUTH_OUTPUT=$(claude auth login --no-open 2>&1 | tee /dev/tty)
    
    # Extract OAuth URL from output (Claude outputs something like "Visit: https://...")
    OAUTH_URL=$(echo "$AUTH_OUTPUT" | grep -E "(Visit:|Open:|URL:)" | grep -oE 'https://[^ ]+' | head -1)
    
    if [ -n "$OAUTH_URL" ]; then
        log ""
        success "=========================================="
        success "OAuth Authentication URL:"
        success "$OAUTH_URL"
        success "=========================================="
        log ""
        log "Please open this URL in your browser to complete authentication."
        log "This container will wait for you to complete the login process."
        log ""
        
        # Wait for authentication to complete (claude auth login will block until done)
        wait
        AUTH_SUCCESS=$?
    else
        error "Failed to extract OAuth URL from Claude output"
        error "You may need to run 'claude auth login' manually"
        exit 1
    fi
else
    log ""
    log "This will open your browser to authenticate with Claude."
    log "After authentication, credentials will be stored for all claude-box sessions."
    log ""
    
    # Run Claude login normally (will open browser)
    claude auth login
    AUTH_SUCCESS=$?
fi

if [ $AUTH_SUCCESS -eq 0 ]; then
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
    if [ "$NON_INTERACTIVE" = "true" ]; then
        error "Make sure you completed the OAuth flow in your browser."
    fi
    exit 1
fi