#!/bin/bash
# ABOUTME: Test the Claude CLI startup fix

echo "🧪 Testing Claude CLI startup fix..."

# Run container with proper authentication 
CONTAINER_ID=$(docker run -d --rm \
    -v "$(pwd):/workspace" \
    -e ANTHROPIC_API_KEY="test-key-for-startup" \
    claude-box:claude-dev)

echo "📦 Started test container: $CONTAINER_ID"

# Wait for startup
echo "⏳ Waiting for container startup..."
sleep 8

# Check tmux sessions
echo "🔍 Checking tmux sessions:"
docker exec $CONTAINER_ID tmux list-sessions || echo "No tmux sessions found"

# Check if log directory was created
echo "🔍 Checking log directory:"
docker exec $CONTAINER_ID ls -la /workspace/.claude-box/logs/ || echo "No logs directory"

# Check if we can attach to the session
echo "🔍 Testing attach to session:"
docker exec $CONTAINER_ID tmux has-session -t claude-session && echo "✅ Claude session exists" || echo "❌ No claude-session found"

# Check startup logs
echo "📋 Container startup logs:"
docker logs $CONTAINER_ID | tail -10

# Test claude-start command directly
echo "🔍 Testing claude-start command:"
timeout 5 docker exec $CONTAINER_ID /bin/bash -c "source ~/.bashrc && claude-start" || echo "claude-start test completed"

# Clean up
echo "🧹 Cleaning up..."
docker stop $CONTAINER_ID

echo "✅ Test completed!"