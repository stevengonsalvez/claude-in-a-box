#!/bin/bash
# ABOUTME: Check what's in the Claude logs

echo "🔍 Checking Claude logs..."

# Run container with proper authentication 
CONTAINER_ID=$(docker run -d --rm \
    -v "$(pwd):/workspace" \
    -e ANTHROPIC_API_KEY="test-key" \
    claude-box:claude-dev)

echo "📦 Started container: $CONTAINER_ID"
sleep 10

# Check log contents
echo "📄 Log file contents:"
docker exec $CONTAINER_ID find /workspace/.claude-box/logs/ -name "*.log" -exec echo "=== {} ===" \; -exec cat {} \;

# Check tmux session content
echo "🖥️  Current tmux session state:"
docker exec $CONTAINER_ID tmux capture-pane -t claude-session -p || echo "Could not capture pane"

# Clean up
docker stop $CONTAINER_ID

echo "✅ Done!"