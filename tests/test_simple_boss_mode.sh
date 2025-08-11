#!/bin/bash
# ABOUTME: Simple test for boss mode prompt appending in startup.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
STARTUP_SCRIPT="$PROJECT_ROOT/docker/claude-dev/scripts/startup.sh"

# Test that startup.sh appends boss mode prompt to user prompt
test_boss_mode_prompt_appending() {
    if [ ! -f "$STARTUP_SCRIPT" ]; then
        echo "❌ Test failed: startup.sh not found at $STARTUP_SCRIPT"
        return 1
    fi

    # Check that startup.sh defines BOSS_MODE_PROMPT
    if grep -q "BOSS_MODE_PROMPT=" "$STARTUP_SCRIPT"; then
        echo "✅ Test 1 passed: startup.sh defines BOSS_MODE_PROMPT"
    else
        echo "❌ Test 1 failed: startup.sh doesn't define BOSS_MODE_PROMPT"
        return 1
    fi

    # Check that startup.sh creates ENHANCED_PROMPT
    if grep -q "ENHANCED_PROMPT=" "$STARTUP_SCRIPT"; then
        echo "✅ Test 2 passed: startup.sh creates ENHANCED_PROMPT"
    else
        echo "❌ Test 2 failed: startup.sh doesn't create ENHANCED_PROMPT"
        return 1
    fi

    # Check that startup.sh uses ENHANCED_PROMPT with correct claude syntax
    if grep -q 'claude --print --output-format text --verbose "${ENHANCED_PROMPT}"' "$STARTUP_SCRIPT"; then
        echo "✅ Test 3 passed: startup.sh uses ENHANCED_PROMPT with correct claude syntax"
    else
        echo "❌ Test 3 failed: startup.sh doesn't use ENHANCED_PROMPT with correct claude syntax"
        return 1
    fi

    # Check that boss mode prompt contains key phrases
    if grep -q "test first" "$STARTUP_SCRIPT" && \
       grep -q "commit early and often" "$STARTUP_SCRIPT" && \
       grep -q "conventional commit format" "$STARTUP_SCRIPT"; then
        echo "✅ Test 4 passed: Boss mode prompt contains required content"
    else
        echo "❌ Test 4 failed: Boss mode prompt missing required content"
        return 1
    fi

    return 0
}

echo "Running simple boss mode prompt appending test..."
echo

test_boss_mode_prompt_appending

echo
echo "Simple boss mode test completed."
