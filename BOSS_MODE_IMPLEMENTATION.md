# Boss Mode Implementation

## Overview

Boss mode is a feature that automatically appends project-specific development guidelines to user prompts when interacting with Claude. This ensures that Claude consistently follows TDD practices, proper commit workflows, and project standards.

## Implementation Details

### Environment Variable
- **Variable**: `CLAUDE_BOSS_MODE`
- **Values**: `"true"` (enabled) or `"false"`/unset (disabled)
- **Scope**: Container environment variable

### Modified Files
- `docker/claude-dev/scripts/claude-logging.sh` - Main implementation
- Added boss mode prompt injection function
- Enhanced both `--print` and `--script` modes

### Boss Mode Prompt Text
When enabled, the following text is appended to user prompts:

```
Ultrathink and understand our project rules, particularly around testing. You must go test first, and you must work in a way that allows for small known-good increments. You must commit when the code is in a working state, and commit early and often. When committing: - Use conventional commit format (feat:, fix:, refactor:, test:, docs:) - Commit after each logical increment (test passes, feature complete, refactor done) - Ensure pre-commit hooks pass before committing - Generate descriptive commit messages that explain the 'what' and 'why' - Never leave code in a broken state between commits
```

## Usage

### Enabling Boss Mode
Boss mode is controlled by the TUI and passed to the container via environment variable:

```bash
# In container environment
export CLAUDE_BOSS_MODE=true
```

### Claude Commands Affected
- `claude-print "query"` - Single queries with logged responses
- `claude-script` - Script mode reading from stdin
- Interactive mode is not affected (as requested)

### Visual Indicators
When boss mode is active, the logs show:
```
🎯 Boss mode: Enhanced with project rules
```

## Testing

### Test Coverage
- ✅ Unit tests for prompt injection function
- ✅ Integration tests with mock Claude CLI
- ✅ Environment variable handling
- ✅ Empty prompt edge cases

### Running Tests
```bash
./tests/run_all_tests.sh
```

## Container Rebuild

After implementing boss mode, rebuild the container:

```bash
./docker/claude-dev/claude-dev.sh --rebuild
```

## Technical Details

### Function Implementation
```bash
inject_boss_mode_prompt() {
    local user_prompt="$1"

    if [ "$CLAUDE_BOSS_MODE" = "true" ]; then
        echo "$user_prompt $BOSS_MODE_PROMPT"
    else
        echo "$user_prompt"
    fi
}
```

### Integration Points
1. **Print Mode**: Enhances query before passing to `claude --print`
2. **Script Mode**: Enhances stdin input before piping to Claude
3. **Logging**: Shows boss mode status in container logs

## Benefits

1. **Consistent Behavior**: Claude always follows project rules
2. **TDD Enforcement**: Emphasizes test-first development
3. **Proper Git Workflow**: Encourages frequent, meaningful commits
4. **Quality Standards**: Maintains code quality and commit message standards
5. **Transparent**: User can see when boss mode is active via logs

## Future Enhancements

- Project-specific boss mode prompts
- Configurable prompt templates
- Boss mode intensity levels
- Integration with project configuration files
