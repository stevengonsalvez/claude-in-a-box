# Pre-commit Setup Documentation

## Overview

The project uses the standard `pre-commit` framework for automated code quality checks. This provides better standardization, easier maintenance, and more flexible configuration than custom bash scripts.

## Migration Summary

- **Old**: Custom bash script at `.git/hooks/pre-commit` (now saved as `.git/hooks/pre-commit.legacy`)
- **New**: Standard pre-commit framework with `.pre-commit-config.yaml`

## Configuration Files

### `.pre-commit-config.yaml` (Single Config)
**Purpose**: Balanced checks for daily development
**Behavior**:
- 🚫 **Blocks commit if**: compilation fails, tests fail, formatting issues
- ⚠️ **Warns but continues if**: clippy issues (shows warning message)
- 🔧 **Auto-fixes**: trailing whitespace, end-of-file, line endings, code formatting

### `.markdownlint.json`
**Purpose**: Relaxed markdown linting rules
**Settings**:
- Line length: 120 characters (instead of 80)
- Disabled: code block language requirements, list numbering strictness

## Usage

### Daily Development (Automatic)
```bash
# Automatic on commit
git commit -m "your message"

# Manual run
pre-commit run --all-files
```

### Individual Hooks
```bash
# Run specific checks
pre-commit run cargo-check
pre-commit run cargo-test
pre-commit run cargo-fmt
pre-commit run cargo-clippy
```

### Skip Hooks When Needed
```bash
# Skip all hooks (emergency only)
git commit --no-verify -m "emergency fix"

# Skip specific hooks
SKIP=cargo-clippy git commit -m "commit with clippy warnings"
SKIP=cargo-test git commit -m "commit without running tests"
```

## What Each Hook Does

### File Quality Hooks
- **trailing-whitespace**: Removes trailing spaces
- **end-of-file-fixer**: Ensures files end with newline
- **check-yaml/toml/json**: Validates file syntax
- **check-added-large-files**: Prevents large file commits (>1MB)
- **check-merge-conflict**: Detects merge conflict markers
- **mixed-line-ending**: Standardizes to LF line endings

### Rust Hooks
- **cargo-check**: Ensures code compiles (BLOCKS commit)
- **cargo-test**: Runs all tests (BLOCKS commit)
- **cargo-fmt**: Formats code (BLOCKS commit, but auto-fixes)
- **cargo-clippy**: Linting (WARNS but doesn't block commit)

## Comparison with Old System

| Feature | Old Bash Script | New Pre-commit Framework |
|---------|----------------|-------------------------|
| **Compilation Check** | ✅ `cargo check` | ✅ `cargo check` |
| **Test Execution** | ✅ `cargo test` | ✅ `cargo test` |
| **Code Formatting** | ✅ `cargo fmt` (auto-fix) | ✅ `cargo fmt` (auto-fix) |
| **Linting** | ⚠️ `cargo clippy` (warn) | ⚠️ `cargo clippy` (warn) |
| **Documentation** | ✅ `cargo doc` | ❌ Removed (too slow/noisy) |
| **File Quality** | ❌ None | ✅ Whitespace, line endings, etc. |
| **Configurability** | ❌ Hard-coded | ✅ Flexible YAML config |
| **Standardization** | ❌ Custom | ✅ Industry standard |

## Benefits of New System

1. **Standardized**: Uses industry-standard pre-commit framework
2. **Flexible**: Easy to add/remove/configure hooks
3. **Maintainable**: No custom bash scripting to maintain
4. **Balanced**: Blocks serious issues, warns on style issues
5. **Auto-updating**: Hooks can be automatically updated
6. **Team-friendly**: Easy to share and replicate across team
7. **Simplified**: Single configuration file, no complexity

## Troubleshooting

### Bypass Pre-commit (Emergency)
```bash
git commit --no-verify -m "emergency fix"
```

### Update Hooks
```bash
pre-commit autoupdate
```

### Clean Install
```bash
pre-commit clean
pre-commit install
```

### Restore Old System (If Needed)
```bash
cp .git/hooks/pre-commit.legacy .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

## Customization

To modify the hooks, edit `.pre-commit-config.yaml`:

```yaml
# Add new hook
- id: my-custom-hook
  name: My Custom Check
  entry: my-command
  language: system
  files: \.rs$

# Modify existing hook
- id: cargo-clippy
  name: Cargo Clippy
  entry: bash -c 'cargo clippy --all-targets --all-features -- -D warnings'  # Make strict
```

## Integration with CI/CD

The same pre-commit configuration can be used in CI:

```yaml
# GitHub Actions example
- name: Run pre-commit
  uses: pre-commit/action@v3.0.1
  with:
    extra_args: --all-files
```

This ensures consistency between local development and CI environments.

## Philosophy

The configuration follows a **"fail fast, warn smart"** philosophy:
- **Block commits** for things that break the build (compilation, tests, formatting)
- **Warn but allow** for style issues that don't break functionality (clippy warnings)
- **Auto-fix** trivial issues (whitespace, formatting) so developers don't have to think about them

This keeps development velocity high while maintaining code quality.
