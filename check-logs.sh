#!/bin/bash
# ABOUTME: Check what's in the Claude logs

set -euo pipefail

echo "🔍 Checking Claude logs..."

# Run container with proper authentication 
CONTAINER_ID=$(docker run -d --rm \
    -v "$(pwd):/workspace" \
    -e ANTHROPIC_API_KEY="test-key" \
    claude-box:claude-dev)

echo "📦 Started container: $CONTAINER_ID"

# Wait for container to be ready (poll instead of fixed sleep)
echo "⏳ Waiting for container to be ready..."
for i in {1..30}; do
    if docker exec "$CONTAINER_ID" test -d /workspace/.claude-box 2>/dev/null; then
        echo "✅ Container is ready after ${i} seconds"
        break
    fi
    if [ $i -eq 30 ]; then
        echo "❌ Container failed to become ready within 30 seconds"
        docker stop "$CONTAINER_ID"
        exit 1
    fi
    sleep 1
done

# Set up cleanup trap to ensure container is always stopped
cleanup() {
    echo "🧹 Cleaning up container..."
    docker stop "$CONTAINER_ID" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# Check log contents
echo "📄 Log file contents:"
docker exec "$CONTAINER_ID" find /workspace/.claude-box/logs/ -name "*.log" -exec echo "=== {} ===" \; -exec cat {} \;

# Check tmux session content
echo "🖥️  Current tmux session state:"
docker exec "$CONTAINER_ID" tmux capture-pane -t claude-session -p || echo "Could not capture pane"

echo "✅ Done!"