# Claude-in-a-Box Credential Storage Guide

## Overview

Claude-in-a-Box maintains an isolated credential storage system in `~/.claude-in-a-box/` to avoid conflicts with host Claude installations and ensure cross-platform compatibility.

## Directory Structure

```
~/.claude-in-a-box/
├── auth/                          # OAuth credential storage
│   ├── .credentials.json         # OAuth tokens (created by claude auth login)
│   └── config.json              # Claude CLI configuration (optional)
├── .claude.json                  # Claude preferences (theme, settings)
├── .env                         # API key storage (if using API key method)
├── config/                      # Application configuration
│   └── config.toml             # Global claude-in-a-box settings
├── worktrees/                   # Git worktrees for sessions
│   ├── by-date/                # Human-readable worktree organization
│   └── by-session/             # Session UUID symlinks
└── logs/                       # Application logs
```

## Authentication Flow

### 1. OAuth Authentication (Recommended)

When you run OAuth authentication:

1. **Auth Container Creation**: A Docker container is created with volume mount:

   ```bash
   -v ~/.claude-in-a-box/auth:/home/claude-user/.claude
   ```

2. **Claude CLI Login**: Inside the container, `claude auth login` runs and:
   - Generates OAuth URL
   - Waits for browser authentication
   - Stores tokens in `/home/claude-user/.claude/.credentials.json`
   - This maps to `~/.claude-in-a-box/auth/.credentials.json` on host

3. **Credential Storage**: The `.credentials.json` file contains:

   ```json
   {
     "accessToken": "...",
     "refreshToken": "...",
     "expiresAt": "..."
   }
   ```

### 2. API Key Authentication

If you choose API key method:

1. **Storage Location**: `~/.claude-in-a-box/.env`
2. **Format**:

   ```env
   ANTHROPIC_API_KEY=sk-ant-xxx...
   ```

## File Mounting Strategy

### During Authentication Setup

The auth container mounts:

```
~/.claude-in-a-box/auth → /home/claude-user/.claude (read-write)
```

This allows the Claude CLI to write credentials during OAuth flow.

### During Development Sessions

Each session container mounts:

1. **OAuth Credentials** (if present):

   ```
   ~/.claude-in-a-box/auth → /home/claude-user/.claude (read-only)
   ```

2. **Claude Preferences** (if present):

   ```
   ~/.claude-in-a-box/.claude.json → /home/claude-user/.claude.json (read-write)
   ```

   The `.claude.json` is mounted read-write to allow Claude CLI to update:
   - Theme preferences (light/dark mode)
   - Editor preferences
   - Other Claude CLI settings

3. **API Key** (if present):

   ```
   ~/.claude-in-a-box/.env → /app/.env (read-only)
   ```

## What Gets Stored Where

### `~/.claude-in-a-box/auth/.credentials.json`

- **Created by**: OAuth authentication flow
- **Contains**: OAuth access tokens, refresh tokens
- **Security**: Read-only mount in sessions
- **Purpose**: Authenticate Claude CLI commands

### `~/.claude-in-a-box/.claude.json`

- **Created by**: Claude CLI on first use or settings change
- **Contains**: User preferences

  ```json
  {
    "theme": "dark",
    "editor": "vim",
    "telemetry": false
  }
  ```

- **Security**: Read-write mount to allow preference updates
- **Purpose**: Persist user preferences across sessions

### `~/.claude-in-a-box/.env`

- **Created by**: API key authentication method
- **Contains**: `ANTHROPIC_API_KEY=...`
- **Security**: Read-only mount, file permissions 600
- **Purpose**: Alternative to OAuth authentication

## Authentication Priority

When a container starts, it checks for authentication in this order:

1. **Mounted OAuth credentials**: `/home/claude-user/.claude/.credentials.json`
2. **Environment variable from .env**: `ANTHROPIC_API_KEY` from mounted `.env`
3. **Runtime environment**: `ANTHROPIC_API_KEY` passed to container

## Security Considerations

1. **Credential Isolation**: All credentials are stored in `~/.claude-in-a-box/`, separate from host Claude installation
2. **Read-Only Mounts**: Credentials are mounted read-only to prevent modification
3. **File Permissions**: Set to 600 (user read/write only)
4. **No Host Access**: Containers cannot access host `~/.claude/` directory

## Common Scenarios

### First-Time Setup

1. Launch claude-in-a-box
2. Presented with auth setup screen
3. Choose OAuth or API key
4. Credentials stored in `~/.claude-in-a-box/`
5. All future sessions use these credentials

### Switching Authentication Methods

```bash
# Remove existing auth
rm -rf ~/.claude-in-a-box/auth
rm -f ~/.claude-in-a-box/.env

# Re-run setup
claude-box
```

### Manual OAuth Re-authentication

```bash
# Run auth setup directly
claude-box auth
```

### Theme Persistence

When you change theme in Claude CLI:

```bash
claude config set theme dark
```

This updates `~/.claude-in-a-box/.claude.json` which persists across sessions.

## Troubleshooting

### Check What's Stored

```bash
# List all claude-in-a-box files
ls -la ~/.claude-in-a-box/

# Check OAuth credentials
ls -la ~/.claude-in-a-box/auth/

# View preferences
cat ~/.claude-in-a-box/.claude.json

# Check API key
cat ~/.claude-in-a-box/.env
```

### Verify Mounts in Container

```bash
# Inside a session container
ls -la ~/.claude/
cat ~/.claude.json
env | grep ANTHROPIC
```

### Reset Everything

```bash
# Complete reset
rm -rf ~/.claude-in-a-box/
```

## Key Points

1. **Complete Isolation**: No interaction with host `~/.claude/` directory
2. **Persistent Preferences**: Theme and settings persist via `.claude.json`
3. **Secure Storage**: Credentials are protected with appropriate permissions
4. **Cross-Platform**: Works consistently on macOS, Linux, and Windows
5. **Container-First**: All authentication happens inside containers
