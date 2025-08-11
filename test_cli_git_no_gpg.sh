#!/bin/bash

# Test CLI git functionality with GPG signing disabled
WORKTREE_PATH="/Users/stevengonsalvez/.claude-in-a-box/worktrees/by-name/claude-in-a-box--claude-fb373816--367093b5"

echo "=== Testing CLI Git with GPG disabled ==="
echo "Worktree: $WORKTREE_PATH"

cd "$WORKTREE_PATH"

# Create a test file
echo "Test content $(date)" > test_file.txt

echo "=== Adding test file ==="
GIT_TERMINAL_PROMPT=0 git add test_file.txt
if [ $? -eq 0 ]; then
    echo "✓ git add succeeded"
else
    echo "✗ git add failed"
    exit 1
fi

echo "=== Committing with --no-gpg-sign ==="
GIT_TERMINAL_PROMPT=0 GIT_ASKPASS=echo git commit --no-gpg-sign -m "test: CLI git commit without GPG signing"
if [ $? -eq 0 ]; then
    echo "✓ git commit succeeded"
else
    echo "✗ git commit failed"
    exit 1
fi

echo "=== Pushing to origin ==="
GIT_TERMINAL_PROMPT=0 GIT_ASKPASS=echo git push origin HEAD
if [ $? -eq 0 ]; then
    echo "✓ git push succeeded"
else
    echo "✗ git push failed"
    echo "Error details:"
    GIT_TERMINAL_PROMPT=0 GIT_ASKPASS=echo git push origin HEAD 2>&1
    exit 1
fi

echo "=== CLI Git test completed successfully ==="

# Clean up test file
rm -f test_file.txt
