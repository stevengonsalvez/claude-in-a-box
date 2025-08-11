#!/bin/bash
# ABOUTME: Integration tests for claude-logging.sh with boss mode functionality

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
CLAUDE_LOGGING_SCRIPT="$PROJECT_ROOT/docker/claude-dev/scripts/claude-logging.sh"

# Mock claude command for testing
create_mock_claude() {
    local mock_dir="$1"
    cat > "$mock_dir/claude" << 'EOF'
#!/bin/bash
# Mock claude command that echoes the prompt it received
if [ -t 0 ]; then
    # Input from arguments
    echo "Mock Claude received: $*"
else
    # Input from stdin
    local stdin_input=$(cat)
    echo "Mock Claude received: $* (stdin: $stdin_input)"
fi
EOF
    chmod +x "$mock_dir/claude"
}

# Test 1: Boss mode disabled - prompt passed through unchanged
test_boss_mode_disabled_integration() {
    local temp_dir=$(mktemp -d)
    local mock_bin="$temp_dir/bin"
    mkdir -p "$mock_bin"
    create_mock_claude "$mock_bin"

    # Set up environment
    export PATH="$mock_bin:$PATH"
    export CLAUDE_BOSS_MODE="false"

    # Test the script
    local result
    result=$("$CLAUDE_LOGGING_SCRIPT" --print "test query" 2>&1 | grep "Mock Claude received" | sed 's/.*Mock Claude received: //' | sed 's/\x1b\[[0-9;]*m//g')

    if [[ "$result" == *"--print --output-format text test query" ]]; then
        echo "✅ Test 1 passed: Boss mode disabled - prompt unchanged"
        cleanup_temp_dir "$temp_dir"
        return 0
    else
        echo "❌ Test 1 failed: Expected 'test query', got '$result'"
        cleanup_temp_dir "$temp_dir"
        return 1
    fi
}

# Test 2: Boss mode enabled - prompt enhanced
test_boss_mode_enabled_integration() {
    local temp_dir=$(mktemp -d)
    local mock_bin="$temp_dir/bin"
    mkdir -p "$mock_bin"
    create_mock_claude "$mock_bin"

    # Set up environment
    export PATH="$mock_bin:$PATH"
    export CLAUDE_BOSS_MODE="true"

    # Test the script
    local result
    result=$("$CLAUDE_LOGGING_SCRIPT" --print "test query" 2>&1 | grep "Mock Claude received" | sed 's/.*Mock Claude received: //' | sed 's/\x1b\[[0-9;]*m//g')

    if [[ "$result" == *"test query Ultrathink and understand our project rules"* ]] && \
       [[ "$result" == *"commit early and often"* ]]; then
        echo "✅ Test 2 passed: Boss mode enabled - prompt enhanced"
        cleanup_temp_dir "$temp_dir"
        return 0
    else
        echo "❌ Test 2 failed: Boss mode prompt not properly injected"
        echo "Result: $result"
        cleanup_temp_dir "$temp_dir"
        return 1
    fi
}

# Test 3: Script mode with boss mode
test_script_mode_with_boss_mode() {
    local temp_dir=$(mktemp -d)
    local mock_bin="$temp_dir/bin"
    mkdir -p "$mock_bin"
    create_mock_claude "$mock_bin"

    # Set up environment
    export PATH="$mock_bin:$PATH"
    export CLAUDE_BOSS_MODE="true"
    unset ANTHROPIC_API_KEY  # Disable real authentication for test

    # Test the script with stdin input
    local result
    result=$(echo "script input" | "$CLAUDE_LOGGING_SCRIPT" --script 2>&1 | grep "Mock Claude received" | sed 's/.*Mock Claude received: //' | sed 's/\x1b\[[0-9;]*m//g')

    if [[ "$result" == *"stdin: script input Ultrathink and understand our project rules"* ]]; then
        echo "✅ Test 3 passed: Script mode with boss mode works"
        cleanup_temp_dir "$temp_dir"
        return 0
    else
        echo "❌ Test 3 failed: Script mode boss mode not working"
        echo "Result: $result"
        cleanup_temp_dir "$temp_dir"
        return 1
    fi
}

cleanup_temp_dir() {
    local temp_dir="$1"
    rm -rf "$temp_dir"
}

# Check if the script exists
if [ ! -f "$CLAUDE_LOGGING_SCRIPT" ]; then
    echo "❌ Claude logging script not found at $CLAUDE_LOGGING_SCRIPT"
    exit 1
fi

echo "Running claude-logging.sh integration tests..."
echo

test_boss_mode_disabled_integration
test_boss_mode_enabled_integration
test_script_mode_with_boss_mode

echo
echo "All integration tests completed."
