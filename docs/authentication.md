# Claude-in-a-Box Authentication System

## Overview

Claude-in-a-Box implements an isolated authentication system that solves the cross-platform compatibility issues between host systems (especially macOS Keychain) and Linux containers. The system provides multiple authentication methods while maintaining security through isolated credential storage.

## Architecture

### Problem Statement

Traditional Claude CLI installations store OAuth tokens in platform-specific locations:
- **macOS**: Keychain (encrypted, not accessible from containers)
- **Linux**: File-based credentials in `~/.claude/`
- **Containers**: Expect file-based credentials, cannot access host Keychain

This creates authentication failures when running Claude CLI inside Docker containers on macOS systems.

### Solution: Isolated Authentication Storage

Claude-in-a-Box uses an isolated authentication directory at `~/.claude-in-a-box/auth/` that:
- Stores credentials in a container-accessible format
- Remains independent of host system authentication
- Provides read-only mounting for security
- Supports multiple authentication methods

## Authentication Flow

### First-Time Setup Detection

The system automatically detects if authentication is needed by checking:

1. **OAuth Credentials**: `~/.claude-in-a-box/auth/.credentials.json`
2. **API Key File**: `~/.claude-in-a-box/.env`
3. **Environment Variable**: `ANTHROPIC_API_KEY`

If none are found, the first-time setup screen is displayed.

### Authentication Methods

#### 1. OAuth (Recommended)
- **Process**: Runs authentication inside a Docker container
- **Storage**: Credentials saved to `~/.claude-in-a-box/auth/`
- **Security**: Read-only mounting prevents credential modification
- **Implementation**: Uses `auth-setup.sh` script in claude-dev container

#### 2. API Key
- **Process**: Manual entry through TUI or CLI
- **Storage**: Saved to `~/.claude-in-a-box/.env` file
- **Format**: `ANTHROPIC_API_KEY=your_key_here`
- **Security**: File permissions restricted to user only

#### 3. Skip Setup
- **Process**: Bypass initial authentication setup
- **Result**: Each container session will prompt for authentication
- **Use Case**: Temporary usage or different credentials per session

## File Structure

### Core Authentication Files

```
~/.claude-in-a-box/
├── auth/                           # OAuth credential storage
│   ├── .credentials.json          # Claude OAuth tokens (generated)
│   └── config.json                # Claude CLI configuration (generated)
├── .env                           # API key storage (if using API key method)
└── config/
    └── config.toml               # Global application config
```

### Implementation Files

#### TUI Components
- **`src/components/auth_setup.rs`**
  - First-time authentication setup screen
  - Method selection (OAuth/API Key/Skip)
  - Progress indicators and error handling
  - Integration with application state machine

#### State Management
- **`src/app/state.rs`**
  - `AuthMethod` enum: OAuth, ApiKey, Skip
  - `AuthSetupState` struct: Current setup state and progress
  - `is_first_time_setup()`: Detection logic for existing authentication
  - Async handlers for authentication operations

#### CLI Integration
- **`src/main.rs`**
  - `claude-box auth` command for CLI-based authentication
  - Argument parsing with clap
  - `run_auth_setup()` function for non-TUI authentication

#### Container Integration
- **`src/docker/session_lifecycle.rs`**
  - Modified mounting logic for isolated auth storage
  - Read-only credential mounting for security
  - Support for both OAuth and API key authentication
  - Environment variable injection for API keys

### Docker Components

#### Authentication Setup Script
- **`docker/claude-dev/scripts/auth-setup.sh`**
  ```bash
  #!/bin/bash
  # ABOUTME: Authentication setup script for claude-in-a-box
  # Runs OAuth login and stores credentials for container sessions
  
  echo "Setting up Claude authentication for claude-in-a-box..."
  echo "This will store credentials in an isolated directory."
  
  # Run Claude CLI login
  claude login
  
  echo "Authentication setup complete!"
  echo "Credentials stored in: /home/claude-user/.claude/"
  echo "These will be available to all claude-in-a-box sessions."
  ```

#### Container Startup Script
- **`docker/claude-dev/scripts/startup.sh`**
  - Updated credential detection logic
  - Support for multiple authentication sources
  - Graceful fallback when credentials are missing

## Security Model

### Credential Isolation
- **Host Independence**: Credentials stored separately from host Claude installation
- **Container Scope**: Credentials only accessible to claude-in-a-box containers
- **Read-Only Mounting**: Prevents accidental credential modification
- **User Permissions**: File permissions restricted to user account

### Authentication Sources Priority
1. **Container-mounted credentials**: `~/.claude-in-a-box/auth/`
2. **Environment variables**: `ANTHROPIC_API_KEY` from `.env` file
3. **Runtime environment**: `ANTHROPIC_API_KEY` environment variable

## Usage Examples

### TUI First-Time Setup
```bash
# Start claude-in-a-box
claude-box

# First-time setup screen appears with options:
# [1] OAuth (Recommended) - Set up using Claude login
# [2] API Key - Enter your Anthropic API key
# [3] Skip - Configure authentication later
```

### CLI Authentication Setup
```bash
# Run authentication setup from command line
claude-box auth

# This launches the OAuth container and guides through setup
```

### Manual API Key Setup
```bash
# Create API key file manually
mkdir -p ~/.claude-in-a-box
echo "ANTHROPIC_API_KEY=your_key_here" > ~/.claude-in-a-box/.env
```

## Container Mounting Strategy

### OAuth Authentication
```rust
// Mount OAuth credentials (read-only)
let claude_box_auth_dir = home_dir.join(".claude-in-a-box/auth");
if claude_box_auth_dir.exists() {
    config = config.with_volume(
        claude_box_auth_dir,
        "/home/claude-user/.claude".to_string(),
        true, // read-only
    );
}
```

### API Key Authentication
```rust
// Mount .env file for API key access
let env_file = home_dir.join(".claude-in-a-box/.env");
if env_file.exists() {
    config = config.with_env_file(&env_file);
}
```

## Troubleshooting

### Common Issues

#### 1. Authentication Not Detected
**Symptoms**: First-time setup keeps appearing
**Solution**: Check file permissions and paths
```bash
ls -la ~/.claude-in-a-box/auth/
ls -la ~/.claude-in-a-box/.env
```

#### 2. Container Authentication Failures
**Symptoms**: Claude CLI reports authentication errors in container
**Solutions**:
- Verify credentials are properly mounted
- Check container logs for detailed error messages
- Re-run authentication setup

#### 3. API Key Not Working
**Symptoms**: API key authentication fails
**Solutions**:
- Verify API key format in `.env` file
- Check API key validity with Anthropic
- Ensure file permissions are correct

#### Real-World Implementation Example

**OAuth Terminal Visibility Fix:**
- **Problem**: OAuth login via `docker run -it` caused TUI garbled input and hidden terminal windows
- **Solution**: Platform-specific terminal spawning with proper window activation and manual refresh capability
- **Files**: `src/app/state.rs:1084-1156`, `src/components/auth_setup.rs`, `src/app/events.rs:49,438-452`
- **Result**: Clean TUI + visible, accessible terminal for OAuth flow with user control

**Key Improvements:**
- **Terminal Activation**: Uses `activate` and `set frontmost` to ensure Terminal window appears
- **Manual Refresh**: Press 'r' to check authentication status anytime
- **Fallback Command**: Press 'c' to get manual CLI command if window doesn't appear
- **Color-coded Messages**: Guide users through the authentication process
- **Cross-platform Support**: Handles macOS, Linux, and Windows differently

### Debug Information

Enable debug logging to troubleshoot authentication issues:
```bash
RUST_LOG=debug claude-box
```

## Future Enhancements

### Planned Features
- **Multiple API Key Support**: Per-project API keys
- **Credential Rotation**: Automatic token refresh
- **Team Authentication**: Shared credential management
- **Backup/Restore**: Credential backup and synchronization

### Security Improvements
- **Encryption**: Encrypt stored credentials
- **Audit Logging**: Track credential access
- **Expiration**: Automatic credential expiration
- **MFA Support**: Multi-factor authentication integration

## Related Documentation

- **[Session Management](session-management.md)**: How authentication integrates with session lifecycle
- **[Container Templates](container-templates.md)**: Container configuration and customization
- **[Configuration](configuration.md)**: Global and project-specific configuration options