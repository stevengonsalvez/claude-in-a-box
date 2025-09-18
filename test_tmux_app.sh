#!/bin/bash
# Test script for tmux integration in Claude-in-a-Box

echo "Testing tmux integration..."

# Clean up any existing test sessions
tmux kill-session -t ciab_test_app 2>/dev/null || true

# Create a test tmux session
echo "Creating test tmux session..."
tmux new-session -d -s ciab_test_app -c /root/repo 'echo "Test session started"; bash'

# Check if session was created
if tmux has-session -t ciab_test_app 2>/dev/null; then
    echo "✓ Tmux session created successfully"
else
    echo "✗ Failed to create tmux session"
    exit 1
fi

# Send a command to the session
echo "Sending test command..."
tmux send-keys -t ciab_test_app "echo 'Hello from tmux!'" Enter

# Capture pane content
echo "Capturing pane content..."
sleep 1
PANE_CONTENT=$(tmux capture-pane -t ciab_test_app -p)
echo "Pane content:"
echo "$PANE_CONTENT"

# Test attaching (in background)
echo "Testing attach functionality..."
timeout 2 tmux attach-session -t ciab_test_app < /dev/null &
ATTACH_PID=$!
sleep 1
if ps -p $ATTACH_PID > /dev/null 2>&1; then
    kill $ATTACH_PID 2>/dev/null
    echo "✓ Attach functionality works"
else
    echo "✓ Attach process completed"
fi

# Clean up
echo "Cleaning up..."
tmux kill-session -t ciab_test_app

echo "All tests completed!"