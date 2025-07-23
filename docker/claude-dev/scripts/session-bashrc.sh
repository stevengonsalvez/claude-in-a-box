#!/bin/bash
# Custom bashrc for Claude-in-a-Box sessions

# Source the default bashrc if it exists
if [ -f /etc/bash.bashrc ]; then
    . /etc/bash.bashrc
fi

# Function to check tmux session status
check_claude_session() {
    if tmux has-session -t "claude-session" 2>/dev/null; then
        echo "Active"
    else
        echo "None"
    fi
}

# Clear screen and show welcome message
clear
echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║              Welcome to Claude-in-a-Box Session                  ║"
echo "╠══════════════════════════════════════════════════════════════════╣"
echo "║                                                                  ║"
echo "║  🚀 Claude CLI is ready to use!                                 ║"
echo "║                                                                  ║"
echo "║  Quick Commands:                                                 ║"
echo "║  • claude-start        - Start/attach to Claude session         ║"
echo "║  • claude              - Shortcut for claude-start              ║"
echo "║  • claude-logs         - View live Claude output                ║"
echo "║  • claude-status       - Check Claude session status            ║"
echo "║  • claude-restart      - Restart Claude session                 ║"
echo "║  • claude-stop         - Stop Claude session                    ║"
echo "║  • exit                - Exit shell (Claude keeps running)      ║"
echo "║                                                                  ║"
echo "║  Tmux Controls (when attached to Claude):                       ║"
echo "║  • Ctrl-b then d       - Detach (Claude keeps running)          ║"
echo "║  • Ctrl-b then [       - Scroll mode (q to exit scroll)         ║"
echo "║                                                                  ║"
echo "║  Session Status:                                                 ║"
echo "║  • Claude Session: $(check_claude_session)                       ║"
echo "║  • Working Directory: $(pwd)                                     ║"
echo "║                                                                  ║"
echo "║  💡 Tip: Just type 'claude' to start chatting!                  ║"
echo "║                                                                  ║"
echo "╚══════════════════════════════════════════════════════════════════╝"
echo

# Set a custom prompt to indicate we're in a Claude-in-a-Box session
export PS1="\[\033[01;32m\][claude-box]\[\033[00m\] \[\033[01;34m\]\w\[\033[00m\] $ "

# Add helpful aliases
alias cls='clear'
alias ll='ls -la'
alias status='docker ps'

# Claude session management functions
claude-start() {
    /app/scripts/claude-session-manager.sh attach
}

claude-logs() {
    /app/scripts/claude-session-manager.sh logs
}

claude-restart() {
    /app/scripts/claude-session-manager.sh restart
}

claude-status() {
    /app/scripts/claude-session-manager.sh status
}

claude-stop() {
    /app/scripts/claude-session-manager.sh stop
}

# Alias for quick access
alias claude='claude-start'

# Export functions so they're available in the shell
export -f claude-start
export -f claude-logs
export -f claude-restart
export -f claude-status
export -f claude-stop
export -f check_claude_session