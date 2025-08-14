#!/bin/bash
# ABOUTME: Test for correct Claude CLI syntax in startup.sh - ensures prompt is passed as positional argument

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
STARTUP_SCRIPT="$PROJECT_ROOT/docker/claude-dev/scripts/startup.sh"

echo "Testing Claude CLI syntax fix in startup.sh..."

# Test 1: Verify startup.sh exists
if [ -f "$STARTUP_SCRIPT" ]; then
    echo "✅ Test 1 passed: startup.sh exists"
else
    echo -e "${RED}❌ Test 1 failed: startup.sh not found at $STARTUP_SCRIPT${NC}"
    exit 1
fi

# Test 2: Verify the INCORRECT syntax is NOT present (claude -p "prompt")
if ! grep -q 'claude -p "${ENHANCED_PROMPT}"' "$STARTUP_SCRIPT"; then
    echo "✅ Test 2 passed: Incorrect syntax 'claude -p \"\${ENHANCED_PROMPT}\"' not found"
else
    echo -e "${RED}❌ Test 2 failed: Incorrect syntax 'claude -p \"\${ENHANCED_PROMPT}\"' still present${NC}"
    exit 1
fi

# Test 3: Verify the CORRECT syntax IS present (claude --print ... "prompt")
if grep -q 'claude --print --output-format text --verbose "${ENHANCED_PROMPT}"' "$STARTUP_SCRIPT"; then
    echo "✅ Test 3 passed: Correct syntax 'claude --print --output-format text --verbose \"\${ENHANCED_PROMPT}\"' found"
else
    echo -e "${RED}❌ Test 3 failed: Correct syntax 'claude --print --output-format text --verbose \"\${ENHANCED_PROMPT}\"' not found${NC}"
    exit 1
fi

# Test 4: Verify the log message is updated to reflect correct syntax
if grep -q 'Running: claude --print' "$STARTUP_SCRIPT"; then
    echo "✅ Test 4 passed: Log message shows correct syntax"
else
    echo -e "${RED}❌ Test 4 failed: Log message doesn't show correct syntax${NC}"
    exit 1
fi

echo -e "${GREEN}All tests passed! Claude CLI syntax is correct.${NC}"
