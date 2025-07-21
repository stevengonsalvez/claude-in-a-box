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
    
    # First check if we have .claude.json config file in mounted auth directory
    if [ -f /home/claude-user/.claude/.claude.json ] && [ -s /home/claude-user/.claude/.claude.json ]; then
        log "Both credentials and configuration files found. Verifying with Claude CLI..."
        # Test existing credentials with timeout
        if timeout 10 claude auth status >/dev/null 2>&1; then
            success "Existing credentials and configuration are valid!"
            success "Authentication setup complete - you can now use claude-box sessions"
            exit 0
        else
            warn "Credentials appear invalid or Claude CLI check failed. Will re-authenticate..."
            rm -f /home/claude-user/.claude/.credentials.json
            rm -f /home/claude-user/.claude/.claude.json
        fi
    else
        warn "Credentials found but missing .claude.json configuration file"
        log "Will attempt to find or recreate .claude.json..."
        
        # Check if .claude.json exists in other locations
        CLAUDE_JSON_FOUND=false
        
        # Check claude-user home directory and copy to mounted auth directory
        if [ -f /home/claude-user/.claude.json ] && [ -s /home/claude-user/.claude.json ]; then
            log "Found .claude.json at: /home/claude-user/.claude.json"
            if cp /home/claude-user/.claude.json /home/claude-user/.claude/.claude.json; then
                success "Configuration copied to ~/.claude-in-a-box/auth/.claude.json"
                success "Authentication setup complete - you can now use claude-box sessions"
                exit 0
            else
                warn "Failed to copy .claude.json configuration file to mounted auth directory"
            fi
        fi
        
        # Check actual HOME directory and copy to mounted auth directory
        if [ -n "$HOME" ] && [ -f "$HOME/.claude.json" ] && [ -s "$HOME/.claude.json" ]; then
            log "Found .claude.json at: $HOME/.claude.json"
            if cp "$HOME/.claude.json" /home/claude-user/.claude/.claude.json; then
                success "Configuration copied to ~/.claude-in-a-box/auth/.claude.json (from HOME)"
                success "Authentication setup complete - you can now use claude-box sessions"
                exit 0
            else
                warn "Failed to copy .claude.json from HOME directory to mounted auth directory"
            fi
        fi
        
        warn ".claude.json not found anywhere. Will proceed without credential validation..."
        log "Starting OAuth to recreate complete authentication setup..."
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
    
    # Wait for credentials file to be created and contain OAuth data
    log "Waiting for OAuth credentials to be written..."
    
    while true; do
        if [ -f /home/claude-user/.claude/.credentials.json ] && [ -s /home/claude-user/.claude/.credentials.json ]; then
            # Check if credentials contain OAuth data
            if grep -q "claudeAiOauth" /home/claude-user/.claude/.credentials.json 2>/dev/null; then
                success "✓ OAuth credentials file created successfully"
                break
            fi
        fi
        sleep 1
        echo -n "."
    done
    echo ""
    
    # Verify credentials were created
    if [ -f /home/claude-user/.claude/.credentials.json ] && [ -s /home/claude-user/.claude/.credentials.json ]; then
        success "Credentials saved to ~/.claude-in-a-box/auth/.credentials.json"
        
        # Now wait for .claude.json to be updated with OAuth configuration
        # Check multiple possible locations where Claude CLI might create the file
        CLAUDE_JSON_FOUND=false
        log "Waiting for .claude.json OAuth configuration to be written..."
        log "Please complete the OAuth flow in your browser if you haven't already."
        log "This session will remain active until OAuth configuration is complete..."
        
        # Function to check if .claude.json has proper OAuth configuration
        check_oauth_config() {
            local file_path="$1"
            if [ ! -f "$file_path" ] || [ ! -s "$file_path" ]; then
                return 1
            fi
            
            # Check for actual OAuth configuration fields (not placeholder data)
            # Look for either userID that's NOT the placeholder, or other OAuth-specific fields
            if grep -q '"userID": "oauth_user_id"' "$file_path" 2>/dev/null; then
                # This is still placeholder data
                return 1
            elif grep -q '"userID":' "$file_path" 2>/dev/null && grep -q '"installMethod":' "$file_path" 2>/dev/null; then
                # Has userID field that's not placeholder + other config = likely real OAuth config
                return 0
            fi
            
            return 1
        }
        
        while true; do
            # Primary location (claude-user home) - copy to mounted auth directory
            if check_oauth_config "/home/claude-user/.claude.json"; then
                log "Found valid .claude.json with OAuth configuration at: /home/claude-user/.claude.json"
                if cp /home/claude-user/.claude.json /home/claude-user/.claude/.claude.json; then
                    success "Configuration saved to ~/.claude-in-a-box/auth/.claude.json"
                    CLAUDE_JSON_FOUND=true
                    break
                else
                    warn "Failed to copy .claude.json configuration file to mounted auth directory"
                fi
            fi
            
            # Alternative: Check if Claude CLI used actual HOME directory and copy to mounted auth directory
            if [ "$CLAUDE_JSON_FOUND" = false ] && [ -n "$HOME" ] && check_oauth_config "$HOME/.claude.json"; then
                log "Found valid .claude.json with OAuth configuration at: $HOME/.claude.json"
                if cp "$HOME/.claude.json" /home/claude-user/.claude/.claude.json; then
                    success "Configuration saved to ~/.claude-in-a-box/auth/.claude.json (from HOME)"
                    CLAUDE_JSON_FOUND=true
                    break
                else
                    warn "Failed to copy .claude.json from HOME directory to mounted auth directory"
                fi
            fi
            
            # Show progress indicator and status
            echo -n "."
            sleep 2  # Check every 2 seconds instead of every 1 second
        done
        echo ""
        
        if [ "$CLAUDE_JSON_FOUND" = false ]; then
            warn ".claude.json not found - Claude CLI configuration may not be available"
            log "Searched locations:"
            log "  - /home/claude-user/.claude.json"
            log "  - $HOME/.claude.json"
            log "Note: Claude CLI currently ignores XDG Base Directory specification"
            log "Available files in claude-user home:"
            ls -la /home/claude-user/ | grep -E "\.(json|credentials)" || log "No config files found"
            log "Available files in HOME ($HOME):"
            ls -la "$HOME/" | grep -E "\.(json|credentials)" || log "No config files found in HOME"
            
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