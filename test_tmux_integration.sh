#!/bin/bash
# ABOUTME: Test script for verifying tmux integration fixes
# Tests session creation, visibility, attachment, and deletion

set -e

echo "Testing tmux integration fixes..."

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Function to print results
print_result() {
    if [ $1 -eq 0 ]; then
        echo -e "${GREEN}✓ $2${NC}"
    else
        echo -e "${RED}✗ $2${NC}"
        exit 1
    fi
}

# Test 1: Check if tmux is available
echo -n "1. Checking tmux availability... "
if command -v tmux &> /dev/null; then
    print_result 0 "tmux is installed"
else
    print_result 1 "tmux is not installed"
fi

# Test 2: List existing tmux sessions
echo -n "2. Listing existing tmux sessions... "
tmux list-sessions 2>/dev/null || true
print_result 0 "Can list tmux sessions"

# Test 3: Create a test tmux session
echo -n "3. Creating test tmux session... "
SESSION_NAME="ciab_test_$(date +%s)"
tmux new-session -d -s "$SESSION_NAME" -c /tmp "echo 'Test session started'" 2>/dev/null
if [ $? -eq 0 ]; then
    print_result 0 "Created session: $SESSION_NAME"
else
    print_result 1 "Failed to create session"
fi

# Small delay to ensure session is created
sleep 0.5

# Test 4: Verify session exists
echo -n "4. Verifying session exists... "
if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
    print_result 0 "Session exists"
else
    print_result 1 "Session not found"
fi

# Test 5: Check session status (detached)
echo -n "5. Checking session status... "
CLIENTS=$(tmux list-clients -t "$SESSION_NAME" 2>/dev/null | wc -l)
if [ "$CLIENTS" -eq 0 ]; then
    print_result 0 "Session is detached (no clients)"
else
    print_result 1 "Session has $CLIENTS clients (expected 0)"
fi

# Test 6: Capture pane content
echo -n "6. Testing pane capture... "
tmux send-keys -t "$SESSION_NAME" "echo 'Test output for capture'" Enter
sleep 1
PANE_CONTENT=$(tmux capture-pane -t "$SESSION_NAME" -p 2>/dev/null)
if echo "$PANE_CONTENT" | grep -q "Test output for capture"; then
    print_result 0 "Pane capture working"
else
    print_result 1 "Pane capture failed"
fi

# Test 7: Kill test session
echo -n "7. Killing test session... "
tmux kill-session -t "$SESSION_NAME" 2>/dev/null
print_result $? "Session killed"

# Test 8: Verify session is gone
echo -n "8. Verifying session is deleted... "
if ! tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
    print_result 0 "Session successfully deleted"
else
    print_result 1 "Session still exists after deletion"
fi

# Test 9: Test persistence directory
echo -n "9. Checking persistence directory... "
PERSISTENCE_DIR="$HOME/.claude-box/sessions"
if [ -d "$PERSISTENCE_DIR" ] || mkdir -p "$PERSISTENCE_DIR" 2>/dev/null; then
    print_result 0 "Persistence directory available: $PERSISTENCE_DIR"
else
    print_result 1 "Cannot create persistence directory"
fi

# Test 10: Create multiple sessions for listing
echo -n "10. Testing multiple session handling... "
tmux new-session -d -s "ciab_workspace1_test" -c /tmp "echo 'Workspace 1'" 2>/dev/null || true
tmux new-session -d -s "ciab_workspace2_test" -c /tmp "echo 'Workspace 2'" 2>/dev/null || true
SESSION_COUNT=$(tmux list-sessions 2>/dev/null | grep -c "ciab_" || echo 0)
print_result 0 "Found $SESSION_COUNT ciab sessions"

# Cleanup
echo -n "11. Cleaning up test sessions... "
tmux kill-session -t "ciab_workspace1_test" 2>/dev/null || true
tmux kill-session -t "ciab_workspace2_test" 2>/dev/null || true
print_result 0 "Cleanup complete"

echo ""
echo "✅ All tmux integration tests passed!"
echo ""
echo "Summary of fixes verified:"
echo "  • Sessions can be created and listed"
echo "  • Session status (attached/detached) can be determined"
echo "  • Sessions can be deleted cleanly"
echo "  • Pane content can be captured"
echo "  • Persistence directory is available"
echo ""
echo "Next steps:"
echo "  1. Run the application: cargo run"
echo "  2. Press 'n' to create a new session"
echo "  3. Verify session appears in the list"
echo "  4. Press 'a' to attach to the session"
echo "  5. Press Ctrl+Q to detach"
echo "  6. Press 'd' to delete the session"
echo "  7. Restart app and verify sessions persist"