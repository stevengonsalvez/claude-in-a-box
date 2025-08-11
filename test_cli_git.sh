#!/bin/bash

# Test CLI git functionality
WORKTREE_PATH="/Users/stevengonsalvez/.claude-in-a-box/worktrees/by-name/claude-in-a-box--claude-fb373816--367093b5"

echo "=== Testing CLI Git in worktree ==="
echo "Worktree: $WORKTREE_PATH"

cd "$WORKTREE_PATH"

echo "=== Current status ==="
git status --porcelain

echo "=== Adding all changes ==="
git add .
if [ $? -eq 0 ]; then
    echo "✓ git add succeeded"
else
    echo "✗ git add failed"
    exit 1
fi

echo "=== Committing changes ==="
git commit -m "test: CLI git commit from script"
if [ $? -eq 0 ]; then
    echo "✓ git commit succeeded"
else
    echo "✗ git commit failed"
    exit 1
fi

echo "=== Pushing to origin ==="
git push origin HEAD
if [ $? -eq 0 ]; then
    echo "✓ git push succeeded"
else
    echo "✗ git push failed"
    echo "Error details:"
    git push origin HEAD 2>&1
    exit 1
fi

echo "=== CLI Git test completed successfully ==="
