#!/bin/bash
# ABOUTME: Complete cleanup script to remove all claude-in-a-box sessions, worktrees, and tmux sessions
# Run this to get a completely fresh start

echo "🧹 Starting complete cleanup of claude-in-a-box sessions..."
echo ""

# 1. Kill all ciab tmux sessions
echo "1. Killing all tmux sessions..."
tmux list-sessions 2>/dev/null | grep "ciab_" | awk -F: '{print $1}' | while read session; do
    echo "   Killing tmux session: $session"
    tmux kill-session -t "$session" 2>/dev/null
done
echo "   ✓ All tmux sessions killed"
echo ""

# 2. Remove all worktrees
echo "2. Removing all worktrees..."
WORKTREE_DIR="$HOME/.claude-in-a-box/worktrees"
if [ -d "$WORKTREE_DIR" ]; then
    echo "   Removing worktree directory: $WORKTREE_DIR"
    rm -rf "$WORKTREE_DIR"
    echo "   ✓ Worktrees removed"
else
    echo "   No worktrees directory found"
fi
echo ""

# 3. Remove session persistence files (both old and new locations)
echo "3. Removing persistence files..."
OLD_SESSIONS="$HOME/.claude-box/sessions"
NEW_SESSIONS="$HOME/.claude-in-a-box/sessions"

if [ -d "$OLD_SESSIONS" ]; then
    echo "   Removing old session files: $OLD_SESSIONS"
    rm -rf "$OLD_SESSIONS"
    echo "   ✓ Old session files removed"
fi

if [ -d "$NEW_SESSIONS" ]; then
    echo "   Removing new session files: $NEW_SESSIONS"
    rm -rf "$NEW_SESSIONS"
    echo "   ✓ New session files removed"
fi
echo ""

# 4. Clean up git worktrees in the current repo
echo "4. Cleaning up git worktrees in current repository..."
cd /Users/stevengonsalvez/d/git/claude-in-a-box
git worktree list | grep -v "(bare)" | tail -n +2 | awk '{print $1}' | while read worktree; do
    echo "   Removing git worktree: $worktree"
    git worktree remove "$worktree" --force 2>/dev/null
done
echo "   ✓ Git worktrees cleaned"
echo ""

# 5. Prune git worktree references
echo "5. Pruning git worktree references..."
git worktree prune
echo "   ✓ Git worktree references pruned"
echo ""

# 6. Clean up any leftover branches
echo "6. Cleaning up claude/* branches..."
git branch | grep "claude/" | while read branch; do
    echo "   Deleting branch: $branch"
    git branch -D "$branch" 2>/dev/null
done
echo "   ✓ Branches cleaned"
echo ""

# 7. Summary
echo "✅ Cleanup complete!"
echo ""
echo "Summary of cleaned items:"
echo "  • All ciab_* tmux sessions killed"
echo "  • Worktree directory removed: ~/.claude-in-a-box/worktrees/"
echo "  • Session persistence removed: ~/.claude-box/sessions/ and ~/.claude-in-a-box/sessions/"
echo "  • Git worktrees removed and pruned"
echo "  • Claude branches deleted"
echo ""
echo "You now have a completely fresh environment!"
echo "Run 'cargo run' to start the application with a clean slate."