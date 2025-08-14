#!/bin/bash
# ABOUTME: Unit tests for boss mode prompt injection function

set -e

# Extract the inject function from the script for testing
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Source the boss mode prompt and function
BOSS_MODE_PROMPT="Ultrathink and understand our project rules, particularly around testing. You must go test first, and you must work in a way that allows for small known-good increments. You must commit when the code is in a working state, and commit early and often. When committing: - Use conventional commit format (feat:, fix:, refactor:, test:, docs:) - Commit after each logical increment (test passes, feature complete, refactor done) - Ensure pre-commit hooks pass before committing - Generate descriptive commit messages that explain the 'what' and 'why' - Never leave code in a broken state between commits"

inject_boss_mode_prompt() {
    local user_prompt="$1"

    if [ "$CLAUDE_BOSS_MODE" = "true" ]; then
        echo "$user_prompt $BOSS_MODE_PROMPT"
    else
        echo "$user_prompt"
    fi
}

# Test 1: Boss mode disabled
test_boss_mode_disabled() {
    export CLAUDE_BOSS_MODE="false"
    local result=$(inject_boss_mode_prompt "test prompt")

    if [ "$result" = "test prompt" ]; then
        echo "✅ Test 1 passed: Boss mode disabled"
        return 0
    else
        echo "❌ Test 1 failed: Expected 'test prompt', got '$result'"
        return 1
    fi
}

# Test 2: Boss mode enabled
test_boss_mode_enabled() {
    export CLAUDE_BOSS_MODE="true"
    local result=$(inject_boss_mode_prompt "test prompt")

    if [[ "$result" == "test prompt $BOSS_MODE_PROMPT" ]]; then
        echo "✅ Test 2 passed: Boss mode enabled"
        return 0
    else
        echo "❌ Test 2 failed: Boss mode prompt not appended correctly"
        return 1
    fi
}

# Test 3: Boss mode with empty prompt
test_boss_mode_empty_prompt() {
    export CLAUDE_BOSS_MODE="true"
    local result=$(inject_boss_mode_prompt "")

    if [[ "$result" == " $BOSS_MODE_PROMPT" ]]; then
        echo "✅ Test 3 passed: Boss mode with empty prompt"
        return 0
    else
        echo "❌ Test 3 failed: Empty prompt handling incorrect"
        return 1
    fi
}

# Test 4: Boss mode undefined (should default to disabled)
test_boss_mode_undefined() {
    unset CLAUDE_BOSS_MODE
    local result=$(inject_boss_mode_prompt "test prompt")

    if [ "$result" = "test prompt" ]; then
        echo "✅ Test 4 passed: Undefined boss mode defaults to disabled"
        return 0
    else
        echo "❌ Test 4 failed: Undefined boss mode not handled correctly"
        return 1
    fi
}

echo "Running boss mode unit tests..."
echo

test_boss_mode_disabled
test_boss_mode_enabled
test_boss_mode_empty_prompt
test_boss_mode_undefined

echo
echo "All unit tests completed."
