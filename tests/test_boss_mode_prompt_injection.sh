#!/bin/bash
# ABOUTME: Tests for boss mode prompt injection functionality

# Test that boss mode appends the correct prompt text to user queries

set -e

# Source the script we're testing (we'll need to extract the logic into a function)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Test data
BOSS_MODE_PROMPT="Ultrathink and understand our project rules, particularly around testing. You must go test first, and you must work in a way that allows for small known-good increments. You must commit when the code is in a working state, and commit early and often. When committing: - Use conventional commit format (feat:, fix:, refactor:, test:, docs:) - Commit after each logical increment (test passes, feature complete, refactor done) - Ensure pre-commit hooks pass before committing - Generate descriptive commit messages that explain the 'what' and 'why' - Never leave code in a broken state between commits"

# Test function to inject boss mode prompt
inject_boss_mode_prompt() {
    local user_prompt="$1"
    local boss_mode_enabled="$2"

    if [ "$boss_mode_enabled" = "true" ]; then
        echo "$user_prompt $BOSS_MODE_PROMPT"
    else
        echo "$user_prompt"
    fi
}

# Test 1: Boss mode disabled - prompt unchanged
test_boss_mode_disabled() {
    local user_prompt="Help me implement a function"
    local result=$(inject_boss_mode_prompt "$user_prompt" "false")

    if [ "$result" = "$user_prompt" ]; then
        echo "✅ Test 1 passed: Boss mode disabled - prompt unchanged"
        return 0
    else
        echo "❌ Test 1 failed: Expected '$user_prompt', got '$result'"
        return 1
    fi
}

# Test 2: Boss mode enabled - prompt has additional text appended
test_boss_mode_enabled() {
    local user_prompt="Help me implement a function"
    local expected="$user_prompt $BOSS_MODE_PROMPT"
    local result=$(inject_boss_mode_prompt "$user_prompt" "true")

    if [ "$result" = "$expected" ]; then
        echo "✅ Test 2 passed: Boss mode enabled - prompt has additional text"
        return 0
    else
        echo "❌ Test 2 failed: Expected additional text to be appended"
        echo "Expected: $expected"
        echo "Got: $result"
        return 1
    fi
}

# Test 3: Empty user prompt with boss mode
test_empty_prompt_with_boss_mode() {
    local user_prompt=""
    local expected=" $BOSS_MODE_PROMPT"
    local result=$(inject_boss_mode_prompt "$user_prompt" "true")

    if [ "$result" = "$expected" ]; then
        echo "✅ Test 3 passed: Empty prompt with boss mode works"
        return 0
    else
        echo "❌ Test 3 failed: Empty prompt handling incorrect"
        echo "Expected: '$expected'"
        echo "Got: '$result'"
        return 1
    fi
}

# Test 4: Boss mode prompt text is exactly as specified
test_boss_mode_prompt_content() {
    local user_prompt="test"
    local result=$(inject_boss_mode_prompt "$user_prompt" "true")

    if [[ "$result" == *"Ultrathink and understand our project rules"* ]] && \
       [[ "$result" == *"test first"* ]] && \
       [[ "$result" == *"commit early and often"* ]] && \
       [[ "$result" == *"conventional commit format"* ]]; then
        echo "✅ Test 4 passed: Boss mode prompt contains required content"
        return 0
    else
        echo "❌ Test 4 failed: Boss mode prompt missing required content"
        echo "Result: $result"
        return 1
    fi
}

# Run all tests
echo "Running boss mode prompt injection tests..."
echo

test_boss_mode_disabled
test_boss_mode_enabled
test_empty_prompt_with_boss_mode
test_boss_mode_prompt_content

echo
echo "All tests completed."
