# Claude-in-a-Box Authentication System

## Overview

Claude-in-a-Box implements a custom OAuth 2.0 authentication system that bypasses the interactive Claude CLI session issue. The system uses Node.js scripts for OAuth flow management and provides secure credential storage isolated from the host system.

## Architecture

### Problem Statement

The original `claude auth login` command has a critical issue:
- Starts an interactive Claude session after OAuth completion
- Blocks the terminal, requiring double Ctrl+C to exit
- Incompatible with non-interactive container environments
- Creates platform-specific credential storage issues (macOS Keychain vs Linux files)

### Solution: Custom OAuth Implementation

Claude-in-a-Box implements a custom OAuth flow that:
- Completes authentication without starting an interactive session
- Uses PKCE (Proof Key for Code Exchange) for enhanced security
- Stores credentials in container-accessible format
- Validates tokens without relying on Claude CLI

## Authentication Components

### OAuth Scripts (Container-side)

#### oauth-start.js
Located at: `docker/claude-dev/scripts/oauth-start.js`

**Purpose**: Initiates OAuth flow by generating authorization URL

**Key Functions**:
- `generateState()`: Creates 32-byte secure random state for CSRF protection
- `generatePKCE()`: Implements RFC 7636 PKCE with S256 challenge method
- `saveState()`: Persists state and code_verifier with 10-minute expiration
- `generateLoginUrl()`: Constructs OAuth URL with all required parameters

**OAuth Constants**:
```javascript
OAUTH_AUTHORIZE_URL: 'https://claude.ai/oauth/authorize'
CLIENT_ID: '9d1c250a-e61b-44d9-88ed-5944d1962f5e'
REDIRECT_URI: 'https://console.anthropic.com/oauth/code/callback'
SCOPES: 'org:create_api_key user:profile user:inference'
```

#### oauth-finish.js
Located at: `docker/claude-dev/scripts/oauth-finish.js`

**Purpose**: Exchanges authorization code for access tokens

**Key Functions**:
- `cleanAuthorizationCode()`: Parses code from various input formats (URL or raw code)
- `verifyState()`: Validates saved state and checks expiration
- `exchangeCodeForTokens()`: POST request to token endpoint with PKCE verification
- `saveCredentials()`: Stores tokens in Claude CLI format + creates `.claude.json`

**Token Exchange Endpoint**:
```javascript
OAUTH_TOKEN_URL: 'https://console.anthropic.com/v1/oauth/token'
```

#### auth-setup.sh
Located at: `docker/claude-dev/scripts/auth-setup.sh`

**Purpose**: Orchestrates the OAuth flow within the container

**Flow**:
1. Checks existing credentials using `jq` for JSON parsing
2. Validates OAuth token structure (`claudeAiOauth.accessToken`)
3. Runs `oauth-start.js` to generate URL
4. Prompts user for authorization code
5. Runs `oauth-finish.js` to complete exchange
6. Verifies credential files were created

### TUI Components (Host-side)

#### State Management (src/app/state.rs)

**Authentication Detection** (Lines 865-891):
```rust
fn needs_authentication_setup(&self) -> bool {
    let has_credentials = auth_dir.join(".credentials.json").exists();
    let has_claude_json = auth_dir.join(".claude.json").exists();

    // For OAuth, need BOTH files AND valid (non-expired) token
    let has_valid_oauth = if has_credentials && has_claude_json {
        Self::is_oauth_token_valid(&auth_dir.join(".credentials.json"))
    } else {
        false
    };

    !has_valid_oauth && !has_api_key && !has_env_api_key
}
```

**OAuth Token Validation** (Lines 894-922):
```rust
fn is_oauth_token_valid(credentials_path: &Path) -> bool {
    // Parse credentials JSON
    // Extract claudeAiOauth.expiresAt
    // Check if current_time < expires_at
}
```

**Container Execution** (Lines 2700-2780):
```rust
// Runs auth container when user selects OAuth
Command::new("docker")
    .args([
        "run", "--rm", "-it",
        "-v", &format!("{}:/home/claude-user/.claude", auth_dir),
        "-e", "AUTH_METHOD=oauth",
        "--entrypoint", "bash",
        "claude-box:claude-dev",
        "-c", "/app/scripts/auth-setup.sh",
    ])
```

## Authentication Flow

### Complete OAuth Flow

```
1. User Selection
   └─> TUI shows auth options → User selects OAuth

2. Container Launch
   └─> TUI spawns Docker container with auth-setup.sh

3. OAuth Initiation
   └─> oauth-start.js generates:
       - State (CSRF protection)
       - Code verifier (PKCE)
       - Code challenge (SHA256 of verifier)
       - OAuth URL with parameters

4. User Authorization
   └─> User opens URL in browser
   └─> Completes Anthropic login
   └─> Authorizes application
   └─> Redirected to callback with code

5. Code Exchange
   └─> User pastes authorization code
   └─> oauth-finish.js:
       - Parses code from input
       - Validates state
       - Exchanges code for tokens
       - Saves credentials

6. Credential Storage
   └─> Creates two files:
       - .credentials.json (OAuth tokens)
       - .claude.json (CLI configuration)

7. TUI Validation
   └─> Checks both files exist
   └─> Validates token not expired
   └─> Enables session creation
```

## File Structure

### Credential Storage

```
~/.claude-in-a-box/auth/
├── .credentials.json          # OAuth tokens
│   └── Structure:
│       {
│         "claudeAiOauth": {
│           "accessToken": "...",
│           "refreshToken": "...",
│           "expiresAt": 1234567890000,
│           "scopes": ["user:inference", "user:profile"],
│           "isMax": true
│         }
│       }
│
├── .claude.json               # CLI configuration
│   └── Structure:
│       {
│         "installMethod": "claude-in-a-box",
│         "autoUpdates": false,
│         "hasCompletedOnboarding": true,
│         "hasTrustDialogAccepted": true,
│         "firstStartTime": "2025-01-01T00:00:00.000Z"
│       }
│
└── .claude_oauth_state.json   # Temporary during OAuth
    └── Structure:
        {
          "state": "random_hex_string",
          "code_verifier": "base64url_string",
          "expires_at": "2025-01-01T00:00:00.000Z"
        }
```

## Security Features

### PKCE Implementation

Prevents authorization code interception attacks:
1. Generate random 32-byte code_verifier
2. Create code_challenge = SHA256(code_verifier)
3. Send challenge with authorization request
4. Send verifier with token exchange
5. Server validates: SHA256(received_verifier) == original_challenge

### State Parameter

Prevents CSRF attacks:
- Random 32-byte hex string
- Saved locally with 10-minute expiration
- Validated during token exchange

### Token Expiration

- OAuth tokens include `expiresAt` timestamp
- TUI validates token hasn't expired before use
- Prevents using stale credentials

### Read-Only Mounting

- Credentials mounted read-only in containers
- Prevents accidental modification
- Maintains security isolation

## Debug Mode

Enable debug logging for OAuth scripts:

```bash
# Set DEBUG environment variable
DEBUG=1 claude-box auth

# Debug output includes:
# - Raw authorization codes
# - Token request/response details
# - State validation steps
# - File creation confirmations
```

## Troubleshooting

### Common Issues

#### OAuth Token Exchange Fails

**Symptom**: "Invalid request format" error

**Solutions**:
1. Ensure you're copying just the code, not the entire URL
2. The oauth-finish.js script handles various formats:
   - Raw code: `abc123xyz`
   - With fragment: `abc123xyz#state`
   - Full URL: `https://console.anthropic.com/oauth/code/callback?code=abc123xyz&state=...`

#### TUI Shows Auth Screen After Successful Auth

**Symptom**: Authentication completes but TUI still prompts for auth

**Cause**: TUI requires BOTH `.credentials.json` AND `.claude.json`

**Solution**: OAuth flow now creates both files automatically

#### Debug Logging Too Verbose

**Symptom**: Too many [DEBUG] messages in output

**Solution**: Debug logging is now conditional - only shows with `DEBUG=1` environment variable

## Testing

### Test Suite

Located at: `docker/claude-dev/scripts/tests/`

**Coverage**:
- OAuth URL generation
- PKCE implementation
- State persistence
- Authorization code parsing
- Token exchange
- Credential storage
- Error handling

**Run Tests**:
```bash
cd docker/claude-dev/scripts/tests
npm test
```

## Implementation History

### Key Commits

1. **Initial OAuth Implementation**: Custom Node.js scripts to bypass interactive session
2. **State Parameter Fix**: Added missing state parameter to token request
3. **Debug Logging**: Made all debug output conditional
4. **URL Parsing**: Enhanced code parsing to handle full callback URLs
5. **TUI Validation**: Create both required files for TUI recognition

### Why Not Use Claude CLI?

The `claude auth login` command has a fundamental issue:
- It starts an interactive Claude session immediately after OAuth
- This blocks the terminal and requires manual interruption
- Our custom implementation completes OAuth and exits cleanly
- No interactive session = no blocking = seamless integration

## Related Documentation

- **[Session Management](session-management.md)**: How authenticated sessions are created
- **[Container Templates](container-templates.md)**: Container configuration for auth
- **[Configuration](configuration.md)**: Authentication configuration options
