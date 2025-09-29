#!/bin/bash

# Test tmux session creation and attachment

echo "Testing tmux session creation..."

# Create a test tmux session
tmux new-session -d -s ciab_test_session -c /tmp bash

# Check if session was created
if tmux has-session -t ciab_test_session 2>/dev/null; then
    echo "✅ Session created successfully"

    # Capture pane content
    echo "Testing pane capture..."
    tmux send-keys -t ciab_test_session "echo 'Hello from tmux session'" Enter
    sleep 0.5
    PANE_CONTENT=$(tmux capture-pane -t ciab_test_session -p)

    if [[ "$PANE_CONTENT" == *"Hello from tmux session"* ]]; then
        echo "✅ Pane capture works"
    else
        echo "❌ Pane capture failed"
    fi

    # Kill the session
    tmux kill-session -t ciab_test_session
    echo "✅ Session killed"
else
    echo "❌ Session creation failed"
fi

echo "Test complete!"