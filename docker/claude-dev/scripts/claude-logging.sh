#!/bin/bash
# ABOUTME: Claude CLI wrapper that logs interactions to stdout for container log capture
# This enables Docker log streaming to show Claude conversations

set -e

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Log function that outputs to stdout for Docker logs
log_to_docker() {
    echo -e "[claude-cli] $1" >&1
}

log_to_docker "${BLUE}🤖 Claude CLI Logger Started${NC}"

# Check for authentication first
AUTH_STATUS="unknown"
if [ -n "$ANTHROPIC_API_KEY" ]; then
    AUTH_STATUS="api-key"
elif [ -f ~/.claude.json ] && [ -s ~/.claude.json ]; then
    AUTH_STATUS="oauth"
elif [ -f ~/.claude/.credentials.json ] && [ -s ~/.claude/.credentials.json ]; then
    AUTH_STATUS="credentials"
else
    log_to_docker "${YELLOW}⚠️  No Claude authentication found${NC}"
    log_to_docker "Available authentication methods:"
    log_to_docker "  1. Set ANTHROPIC_API_KEY environment variable"
    log_to_docker "  2. Mount ~/.claude.json (OAuth)"
    log_to_docker "  3. Mount ~/.claude/.credentials.json"
    exit 1
fi

log_to_docker "${GREEN}✅ Authentication detected: ${AUTH_STATUS}${NC}"

# Configure Claude to avoid interactive prompts only if skip permissions is enabled
if [[ "$CLAUDE_CONTINUE_FLAG" == *"--dangerously-skip-permissions"* ]]; then
    log_to_docker "${BLUE}🔓 Setting trust dialog acceptance (skip permissions enabled)${NC}"
    # Use direct binary to avoid triggering our wrapper's --dangerously-skip-permissions flag
    /home/claude-user/.npm-global/bin/claude config set hasTrustDialogAccepted true >/dev/null 2>&1 || true
else
    log_to_docker "${BLUE}🔒 Trust dialog will be shown as needed (permissions enabled)${NC}"
fi

# Set up environment for better logging
export CLAUDE_LOG_LEVEL=info
export CLAUDE_DISABLE_NONESSENTIAL_TRAFFIC=true

# Function to run Claude in different modes
run_claude_with_logging() {
    local mode="$1"
    shift
    
    case "$mode" in
        "interactive")
            log_to_docker "${BLUE}🚀 Starting Claude CLI in interactive mode${NC}"
            log_to_docker "To see Claude responses in these logs, use non-interactive mode"
            log_to_docker "Commands: 'claude-print' for single queries, 'claude-script' for scripts"
            
            # For interactive mode, we can't capture the TTY output easily
            # but we log the session start/end
            if [ -n "$CLAUDE_CONTINUE_FLAG" ]; then
                # Split CLAUDE_CONTINUE_FLAG into array elements safely
                IFS=' ' read -ra CLAUDE_FLAGS <<< "$CLAUDE_CONTINUE_FLAG"
                claude "${CLAUDE_FLAGS[@]}" "$@"
            else
                claude "$@"
            fi
            log_to_docker "${BLUE}📝 Claude interactive session ended${NC}"
            ;;
            
        "print")
            if [ $# -eq 0 ]; then
                log_to_docker "${YELLOW}Usage: claude-print \"your question here\"${NC}"
                return 1
            fi
            
            local query="$*"
            log_to_docker "${BLUE}👤 User: ${query}${NC}"
            
            # Use claude with --print flag to get output we can capture
            local response
            if [ -n "$CLAUDE_CONTINUE_FLAG" ]; then
                # Split CLAUDE_CONTINUE_FLAG into array elements safely
                IFS=' ' read -ra CLAUDE_FLAGS <<< "$CLAUDE_CONTINUE_FLAG"
                response=$(claude "${CLAUDE_FLAGS[@]}" --print "$query" 2>&1)
            else
                response=$(claude --print "$query" 2>&1)
            fi
            if [ $? -eq 0 ]; then
                log_to_docker "${GREEN}🤖 Claude: ${response}${NC}"
            else
                log_to_docker "${YELLOW}❌ Claude error: ${response}${NC}"
            fi
            ;;
            
        "script")
            # For script mode - pipe input and capture output
            log_to_docker "${BLUE}📄 Running Claude script mode (reading from stdin)${NC}"
            
            local response
            if [ -n "$CLAUDE_CONTINUE_FLAG" ]; then
                # Split CLAUDE_CONTINUE_FLAG into array elements safely
                IFS=' ' read -ra CLAUDE_FLAGS <<< "$CLAUDE_CONTINUE_FLAG"
                response=$(claude "${CLAUDE_FLAGS[@]}" --print 2>&1)
            else
                response=$(claude --print 2>&1)
            fi
            if [ $? -eq 0 ]; then
                log_to_docker "${GREEN}🤖 Claude: ${response}${NC}"
            else
                log_to_docker "${YELLOW}❌ Claude error: ${response}${NC}"
            fi
            ;;
            
        *)
            log_to_docker "${YELLOW}Unknown mode: $mode${NC}"
            return 1
            ;;
    esac
}

# Main execution logic
if [ $# -eq 0 ]; then
    # Default to interactive mode
    run_claude_with_logging "interactive"
else
    case "$1" in
        "--print"|"-p")
            shift
            run_claude_with_logging "print" "$@"
            ;;
        "--script")
            shift
            run_claude_with_logging "script" "$@"
            ;;
        "--help"|"-h")
            log_to_docker "Claude CLI Logging Wrapper"
            log_to_docker "Usage:"
            log_to_docker "  claude-logging                    # Interactive mode (responses not logged)"
            log_to_docker "  claude-logging --print \"query\"    # Single query with logged response"
            log_to_docker "  claude-logging --script           # Script mode (reads from stdin)"
            ;;
        *)
            # Pass through to claude directly
            run_claude_with_logging "interactive" "$@"
            ;;
    esac
fi