#!/bin/bash
# ABOUTME: Run all tests for boss mode functionality

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "🧪 Running all boss mode tests..."
echo "=================================="
echo

# Run unit tests
echo "📋 Running unit tests..."
"$SCRIPT_DIR/test_boss_mode_unit.sh"
echo

# Run original prompt injection tests
echo "📋 Running prompt injection tests..."
"$SCRIPT_DIR/test_boss_mode_prompt_injection.sh"
echo

echo "✅ All tests completed successfully!"
echo
echo "📝 Summary:"
echo "- Boss mode prompt injection function works correctly"
echo "- Environment variable CLAUDE_BOSS_MODE controls behavior"
echo "- Prompt text includes TDD and commit guidelines"
echo "- Ready for container rebuild and deployment"
