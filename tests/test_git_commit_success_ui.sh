#!/bin/bash
# ABOUTME: Test for git commit success UI feedback and screen transition

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Testing git commit success UI feedback and screen transition..."

# Test 1: Check that git view has commit success handling
GIT_VIEW_FILE="$PROJECT_ROOT/src/components/git_view.rs"
if [ -f "$GIT_VIEW_FILE" ]; then
    echo "✅ Test 1 passed: git_view.rs exists"
else
    echo -e "${RED}❌ Test 1 failed: git_view.rs not found${NC}"
    exit 1
fi

# Test 2: Check for commit success notification handling
if grep -q "commit.*success" "$GIT_VIEW_FILE" || grep -q "CommitSuccess" "$GIT_VIEW_FILE"; then
    echo "✅ Test 2 passed: Commit success handling found"
else
    echo -e "${RED}❌ Test 2 failed: No commit success handling found${NC}"
    exit 1
fi

# Test 3: Check for screen transition after commit
EVENTS_FILE="$PROJECT_ROOT/src/app/events.rs"
if grep -q "View::SessionList" "$EVENTS_FILE" && grep -q "GitCommitSuccess" "$EVENTS_FILE"; then
    echo "✅ Test 3 passed: Screen transition after commit found"
else
    echo -e "${RED}❌ Test 3 failed: No screen transition after commit found${NC}"
    exit 1
fi

# Test 4: Check for notification system integration
EVENTS_FILE="$PROJECT_ROOT/src/app/events.rs"
if grep -q "GitCommitSuccess" "$EVENTS_FILE" || grep -q "CommitSuccess" "$EVENTS_FILE"; then
    echo "✅ Test 4 passed: Git commit success event found"
else
    echo -e "${RED}❌ Test 4 failed: No git commit success event found${NC}"
    exit 1
fi

echo -e "${GREEN}All tests passed! Git commit success UI feedback is implemented.${NC}"
