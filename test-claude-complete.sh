#!/bin/bash
# ABOUTME: Comprehensive test script for Claude-in-a-Box session management
# Tests the complete user workflow including tmux session handling

set -e

echo "🧪 Testing Complete Claude-in-a-Box Workflow..."
echo "================================================"

# Build the Docker image first
echo "🔨 Building Docker image..."
docker build -t claude-box:claude-dev docker/claude-dev

# Run container with proper authentication 
echo "📦 Starting test container..."
CONTAINER_ID=$(docker run -d --rm \
    -v "$(pwd):/workspace" \
    -e ANTHROPIC_API_KEY="${ANTHROPIC_API_KEY:-test-key-for-startup}" \
    claude-box:claude-dev)

echo "📦 Container ID: $CONTAINER_ID"

# Helper function to run commands in container
run_in_container() {
    docker exec $CONTAINER_ID "$@"
}

# Helper function to run interactive commands
run_interactive() {
    docker exec -it $CONTAINER_ID "$@"
}

# Wait for startup
echo "⏳ Waiting for container startup..."
sleep 10

echo ""
echo "🔍 Phase 1: Initial Container State"
echo "-----------------------------------"

# Check if tmux session was created
echo "📋 Checking tmux sessions:"
run_in_container tmux list-sessions || echo "No tmux sessions found"

# Check if log directory was created
echo ""
echo "📁 Checking log directory:"
run_in_container ls -la /workspace/.claude-box/logs/ || echo "No logs directory"

# Check container logs
echo ""
echo "📋 Container startup logs (last 20 lines):"
docker logs $CONTAINER_ID | tail -20

echo ""
echo "🔍 Phase 2: Testing Claude Commands"
echo "-----------------------------------"

# Test claude-status command
echo "📊 Testing claude-status:"
run_in_container bash -c "source ~/.bashrc && claude-status"

# Check if claude session exists
echo ""
echo "🔍 Checking if claude-session exists:"
if run_in_container tmux has-session -t claude-session 2>/dev/null; then
    echo "✅ Claude session exists"
    
    # Check what's running in the session
    echo "📋 Claude session pane contents:"
    run_in_container tmux capture-pane -t claude-session -p | tail -20
else
    echo "❌ No claude-session found"
fi

# Test attaching from non-tmux context
echo ""
echo "🔍 Phase 3: Testing claude-start (non-tmux context)"
echo "---------------------------------------------------"
echo "Testing claude-start command (5 second timeout)..."
timeout 5 run_in_container bash -c "source ~/.bashrc && claude-start" || true

# Test from within tmux context (simulating user shell)
echo ""
echo "🔍 Phase 4: Testing claude-start (from tmux context)"
echo "----------------------------------------------------"
echo "Creating user tmux session and testing claude-start..."
run_in_container tmux new-session -d -s test-session "source ~/.bashrc && claude-start; sleep 5"
sleep 3
echo "📋 Test session output:"
run_in_container tmux capture-pane -t test-session -p | tail -10 || echo "Could not capture test session"
run_in_container tmux kill-session -t test-session 2>/dev/null || true

# Check logs
echo ""
echo "🔍 Phase 5: Checking Claude Logs"
echo "---------------------------------"
echo "📋 Available log files:"
run_in_container ls -la /workspace/.claude-box/logs/ || echo "No logs found"

echo ""
echo "📋 Latest Claude log content (if exists):"
run_in_container bash -c 'latest_log=$(ls -t /workspace/.claude-box/logs/claude-*.log 2>/dev/null | head -n1); if [ -n "$latest_log" ]; then echo "Log file: $latest_log"; tail -30 "$latest_log"; else echo "No Claude logs found"; fi'

# Test restart functionality
echo ""
echo "🔍 Phase 6: Testing claude-restart"
echo "----------------------------------"
run_in_container bash -c "source ~/.bashrc && claude-restart" &
sleep 5

echo "📊 Status after restart:"
run_in_container bash -c "source ~/.bashrc && claude-status"

# Final status check
echo ""
echo "🔍 Phase 7: Final Status Check"
echo "------------------------------"
echo "📋 All tmux sessions:"
run_in_container tmux list-sessions || echo "No tmux sessions"

echo ""
echo "📋 Process list (claude-related):"
run_in_container ps aux | grep -E "(claude|tmux)" | grep -v grep || echo "No claude processes found"

# Cleanup
echo ""
echo "🧹 Cleaning up..."
docker stop $CONTAINER_ID

echo ""
echo "✅ Test completed!"
echo ""
echo "📝 Summary:"
echo "- Container startup: SUCCESS"
echo "- Tmux session management: Check output above"
echo "- Claude CLI status: Check logs above"
echo "- User commands: claude-start, claude-status, claude-restart, claude-logs"