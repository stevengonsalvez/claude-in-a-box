#!/bin/bash
# ABOUTME: Quick test script to verify Claude auto-start functionality

# Test the container auto-start behavior
echo "🧪 Testing Claude-in-a-Box auto-start functionality..."

# Run container in background for testing
CONTAINER_ID=$(docker run -d --rm \
    -v "$(pwd):/workspace" \
    -v "$HOME/.claude:/home/claude-user/.claude" \
    -e ANTHROPIC_API_KEY="$ANTHROPIC_API_KEY" \
    claude-box:claude-dev)

echo "📦 Started test container: $CONTAINER_ID"

# Wait a moment for startup
echo "⏳ Waiting for container startup..."
sleep 5

# Check if tmux session was created
echo "🔍 Checking if Claude session was auto-started..."
docker exec $CONTAINER_ID tmux list-sessions

# Check if logs are being created
echo "🔍 Checking log directory..."
docker exec $CONTAINER_ID ls -la /workspace/.claude-box/logs/

# Test our claude-start script
echo "🔍 Testing claude-start command..."
docker exec $CONTAINER_ID /bin/bash -c "source ~/.bashrc && type claude-start"

# Show container logs
echo "📋 Container startup logs:"
docker logs $CONTAINER_ID

# Clean up
echo "🧹 Cleaning up test container..."
docker stop $CONTAINER_ID

echo "✅ Test completed!"