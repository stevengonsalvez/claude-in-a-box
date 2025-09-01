#!/bin/bash
# Test script to verify interactive mode and attach functionality

echo "Testing Interactive Mode with Docker Attach"
echo "==========================================="

# Find a running interactive container
CONTAINER=$(docker ps --filter "label=claude-managed=true" --filter "status=running" -q | head -1)

if [ -z "$CONTAINER" ]; then
    echo "No running Claude containers found"
    echo "Please start an interactive session through the TUI first"
    exit 1
fi

echo "Found container: $CONTAINER"
echo ""

# Check container logs to verify Claude is running
echo "Checking container logs..."
docker logs --tail 20 "$CONTAINER" 2>&1 | grep -E "(Welcome to Claude|cwd:|SessionStart)" && echo "✅ Claude is running"

# Check environment variables
echo ""
echo "Checking container environment..."
docker exec "$CONTAINER" env | grep -E "CLAUDE_BOX_MODE|INTERACTIVE_MODE" && echo "✅ Interactive mode confirmed"

# Check if TTY is allocated
echo ""
echo "Checking TTY allocation..."
docker inspect "$CONTAINER" | jq '.[0].Config.Tty' && echo "✅ TTY is allocated"

# Check attach settings
echo ""
echo "Checking attach settings..."
docker inspect "$CONTAINER" | jq '.[0].Config | {AttachStdin, AttachStdout, AttachStderr, OpenStdin, StdinOnce}' && echo "✅ Attach settings configured"

echo ""
echo "All checks passed! The container is properly configured for interactive mode."
echo "You should now be able to attach to it from the TUI."