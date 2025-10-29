# Amazon Q CLI Integration Plan for Claude-in-a-Box

## Executive Summary

This document outlines the comprehensive plan to integrate Amazon Q CLI into the claude-in-a-box application, enabling users to choose between Claude AI and Amazon Q Developer as their AI coding assistant. The integration will support both interactive mode and boss mode for Amazon Q, with proper authentication handling and Docker containerization.

---

## 1. Overview

### 1.1 What is Amazon Q Developer CLI?

Amazon Q Developer CLI is AWS's AI-powered coding assistant for the command line, offering:
- Natural language chat interface (`q chat`)
- Command translation (`q translate`)
- Agentic coding assistance
- Integration with AWS services
- Free tier with Builder ID and Pro tier with IAM Identity Center

### 1.2 Integration Goals

1. **Multi-Provider Support**: Allow users to choose between Claude CLI and Amazon Q CLI
2. **Unified Experience**: Maintain consistent TUI and workflow across both providers
3. **Boss Mode Support**: Enable Amazon Q to work in boss mode with auto-enhanced prompts
4. **Docker Integration**: Package Amazon Q CLI in Docker containers alongside Claude CLI
5. **Authentication Management**: Handle both Claude and Amazon Q authentication flows
6. **Configuration Flexibility**: Allow per-project and per-session AI provider selection

---

## 2. Technical Architecture

### 2.1 Installation Method

Amazon Q CLI is distributed as:
- **Linux**: `.deb` package (Ubuntu/Debian) or zip archive
- **Installation URL**: `https://desktop-release.q.us-east-1.amazonaws.com/latest/amazon-q.deb`
- **Binary**: Rust-based, not npm package
- **Commands**: `q` (main CLI) and `qterm` (terminal integration)

### 2.2 Authentication Methods

Amazon Q CLI supports two authentication methods:

1. **Builder ID (Free Tier)**
   ```bash
   q login --license free
   ```
   - Free tier access
   - Personal AWS Builder ID account
   - Basic features

2. **IAM Identity Center (Pro Tier)**
   ```bash
   q login --license pro \
     --identity-provider https://company.awsapps.com/start \
     --region us-east-1
   ```
   - $19/month per user
   - Enterprise SSO integration
   - Advanced features

### 2.3 Key Commands

- `q login`: Authenticate with Amazon Q
- `q chat`: Interactive chat session
- `q chat "prompt"`: Single prompt execution (for boss mode)
- `q chat --no-interactive --trust-all-tools`: Non-interactive mode
- `q translate`: Natural language to shell commands
- `q doctor`: Troubleshoot issues
- `q logout`: Log out

### 2.4 Configuration Storage

Amazon Q stores configuration in:
- `~/.aws/q/`: Configuration directory
- `~/.aws/q/config`: CLI configuration
- System keychain: Authentication tokens

---

## 3. Docker Integration Plan

### 3.1 Dockerfile Modifications

**File**: `docker/claude-dev/Dockerfile`

#### 3.1.1 Add Amazon Q Installation

```dockerfile
# Install Amazon Q CLI (after AWS CLI installation at line ~78)
RUN curl -fsSL "https://desktop-release.q.us-east-1.amazonaws.com/latest/amazon-q.deb" -o "/tmp/amazon-q.deb" && \
    sudo dpkg -i /tmp/amazon-q.deb || sudo apt-get install -f -y && \
    rm /tmp/amazon-q.deb && \
    # Verify installation
    q --version || echo "Amazon Q CLI installed (requires authentication to use)"

# Alternative: Install from zip for non-Debian systems
# RUN curl -fsSL "https://desktop-release.q.us-east-1.amazonaws.com/latest/q-x86_64-linux.zip" -o "/tmp/amazon-q.zip" && \
#     unzip /tmp/amazon-q.zip -d /tmp/amazon-q && \
#     sudo install /tmp/amazon-q/q /usr/local/bin/q && \
#     sudo install /tmp/amazon-q/qterm /usr/local/bin/qterm && \
#     rm -rf /tmp/amazon-q.zip /tmp/amazon-q
```

#### 3.1.2 Create Amazon Q Configuration Directory

```dockerfile
# Create Amazon Q configuration directory (after line ~84)
RUN mkdir -p /home/claude-user/.aws/q && \
    chown -R claude-user:claude-user /home/claude-user/.aws
```

### 3.2 Volume Mounts

**File**: `docker/claude-dev/claude-dev.sh` (container launch script)

Add volume mounts for Amazon Q authentication:

```bash
# Mount Amazon Q configuration
-v "$HOME/.aws/q:/home/claude-user/.aws/q:rw"
```

### 3.3 Authentication File Syncing

Amazon Q authentication requires:
1. Initial authentication on host or via device flow in container
2. Sync authentication files to shared location
3. Mount shared authentication in containers

**Host Setup Script** (new file: `scripts/setup-amazon-q.sh`):

```bash
#!/bin/bash
# Setup Amazon Q authentication for claude-in-a-box

echo "Setting up Amazon Q authentication..."
echo ""
echo "Choose authentication method:"
echo "1. Builder ID (Free)"
echo "2. IAM Identity Center (Pro)"
read -p "Selection [1-2]: " AUTH_METHOD

if [ "$AUTH_METHOD" = "1" ]; then
    q login --license free --use-device-flow
elif [ "$AUTH_METHOD" = "2" ]; then
    read -p "Identity Provider URL: " IDP_URL
    read -p "Region [us-east-1]: " REGION
    REGION=${REGION:-us-east-1}
    q login --license pro --identity-provider "$IDP_URL" --region "$REGION" --use-device-flow
else
    echo "Invalid selection"
    exit 1
fi

# Copy authentication to claude-in-a-box directory
mkdir -p ~/.claude-in-a-box/amazon-q
if [ -d ~/.aws/q ]; then
    cp -r ~/.aws/q/* ~/.claude-in-a-box/amazon-q/
    echo "✅ Amazon Q authentication configured"
else
    echo "⚠️  Authentication may have failed. Check ~/.aws/q"
fi
```

---

## 4. Startup Script Integration

### 4.1 Environment Variables

**New Environment Variables**:

```bash
# AI Provider selection
AI_PROVIDER="claude"           # Options: "claude", "amazonq", "both"

# Amazon Q specific
AMAZON_Q_LICENSE="free"        # Options: "free", "pro"
AMAZON_Q_IDP=""               # IAM Identity Center URL (for pro)
AMAZON_Q_REGION="us-east-1"   # AWS region for Amazon Q

# Boss mode AI provider
BOSS_MODE_PROVIDER="claude"   # Which AI to use in boss mode
```

### 4.2 Startup Script Modifications

**File**: `docker/claude-dev/scripts/startup.sh`

#### 4.2.1 Authentication Detection

Add after Claude authentication detection (line ~80):

```bash
# Check for Amazon Q authentication
AMAZONQ_AUTH_OK=false
if [ -d /home/claude-user/.aws/q ] && [ -f /home/claude-user/.aws/q/config ]; then
    if q doctor > /dev/null 2>&1; then
        AMAZONQ_AUTH_OK=true
        AUTH_SOURCES+=("Amazon Q (~/.aws/q)")
        log "✅ Amazon Q authentication detected"
    fi
fi

# Determine available AI providers
AVAILABLE_PROVIDERS=()
if [ "${AUTH_OK}" = "true" ]; then
    AVAILABLE_PROVIDERS+=("claude")
fi
if [ "${AMAZONQ_AUTH_OK}" = "true" ]; then
    AVAILABLE_PROVIDERS+=("amazonq")
fi

# Default AI provider selection
if [ -z "${AI_PROVIDER}" ]; then
    if [ "${#AVAILABLE_PROVIDERS[@]}" -gt 0 ]; then
        AI_PROVIDER="${AVAILABLE_PROVIDERS[0]}"
        log "Defaulting to AI provider: ${AI_PROVIDER}"
    fi
fi
```

#### 4.2.2 Boss Mode Integration

Modify boss mode execution (around line ~139):

```bash
# Handle boss mode execution
if [ "${CLAUDE_BOX_MODE}" = "boss" ] && [ -n "${CLAUDE_BOX_PROMPT}" ]; then
    # Create log directory
    mkdir -p /workspace/.claude-box/logs

    # Determine which AI provider to use
    BOSS_AI_PROVIDER="${BOSS_MODE_PROVIDER:-${AI_PROVIDER}}"

    # Boss mode prompt enhancement
    BOSS_MODE_PROMPT="Ultrathink and understand our project rules, particularly around testing. You must go test first, and you must work in a way that allows for small known-good increments. You must commit when the code is in a working state, and commit early and often. When committing: - Use conventional commit format (feat:, fix:, refactor:, test:, docs:) - Commit after each logical increment (test passes, feature complete, refactor done) - Generate descriptive commit messages that explain the 'what' and 'why' - Never leave code in a broken state between commits"

    # Append boss mode prompt to user prompt
    ENHANCED_PROMPT="${CLAUDE_BOX_PROMPT} ${BOSS_MODE_PROMPT}"

    if [ "${BOSS_AI_PROVIDER}" = "amazonq" ]; then
        if [ "${AMAZONQ_AUTH_OK}" = "true" ]; then
            success "✅ Amazon Q authentication detected - executing boss mode"
            log "🤖 Executing boss mode prompt with Amazon Q..."
            log "Prompt: ${CLAUDE_BOX_PROMPT}"

            # Execute Amazon Q in non-interactive mode
            exec q chat --no-interactive --trust-all-tools "${ENHANCED_PROMPT}"
        else
            error "❌ Boss mode with Amazon Q requires authentication!"
            error "Run 'q login' to authenticate"
            exit 1
        fi
    elif [ "${BOSS_AI_PROVIDER}" = "claude" ]; then
        # Original Claude implementation
        if [ "${AUTH_OK}" = "true" ]; then
            success "✅ Authentication detected - Claude will work immediately"
            log "🤖 Executing boss mode prompt with Claude..."
            log "Prompt: ${CLAUDE_BOX_PROMPT}"
            exec claude --print --output-format stream-json --verbose "${ENHANCED_PROMPT}" $CLI_ARGS
        else
            error "❌ Boss mode with Claude requires authentication!"
            exit 1
        fi
    else
        error "❌ Unknown AI provider: ${BOSS_AI_PROVIDER}"
        error "Valid options: claude, amazonq"
        exit 1
    fi
fi
```

#### 4.2.3 Interactive Mode Integration

Add Amazon Q commands to interactive shell (around line ~183):

```bash
# If no command specified, run interactive shell
if [ $# -eq 0 ]; then
    mkdir -p /workspace/.claude-box/logs

    success "Container environment ready!"

    # Show available AI providers
    if [ "${#AVAILABLE_PROVIDERS[@]}" -gt 0 ]; then
        success "✅ Available AI Providers: ${AVAILABLE_PROVIDERS[*]}"

        if [[ " ${AVAILABLE_PROVIDERS[*]} " =~ " claude " ]]; then
            success "📝 Claude Commands:"
            success "   • claude-ask \"question\" - Ask Claude"
            success "   • claude - Interactive Claude CLI"
        fi

        if [[ " ${AVAILABLE_PROVIDERS[*]} " =~ " amazonq " ]]; then
            success "🔧 Amazon Q Commands:"
            success "   • q chat - Interactive Amazon Q chat"
            success "   • q chat \"question\" - Ask Amazon Q"
            success "   • q translate \"natural language\" - Convert to shell command"
            success "   • q doctor - Troubleshoot issues"
        fi
    else
        warn "⚠️  No AI authentication detected"
    fi

    log "Starting interactive shell..."
    if [ -t 0 ]; then
        exec bash
    else
        log "No TTY detected, keeping container alive..."
        exec sleep infinity
    fi
fi
```

### 4.3 New Helper Scripts

**File**: `docker/claude-dev/scripts/amazon-q-commands.sh`

```bash
#!/bin/bash
# Amazon Q convenience commands for claude-in-a-box

# Ask Amazon Q and log the response
q-ask() {
    local question="$*"
    local logfile="/workspace/.claude-box/logs/amazon-q-$(date +%Y%m%d-%H%M%S).log"

    echo "[$(date '+%Y-%m-%d %H:%M:%S')] Question: $question" | tee "$logfile"
    echo "---" | tee -a "$logfile"

    q chat "$question" 2>&1 | tee -a "$logfile"

    echo "---" | tee -a "$logfile"
    echo "Logged to: $logfile"
}

# Translate natural language to command
q-translate() {
    local prompt="$*"
    q translate "$prompt"
}

# Interactive Amazon Q with logging
q-interactive() {
    local logfile="/workspace/.claude-box/logs/amazon-q-session-$(date +%Y%m%d-%H%M%S).log"
    echo "Starting Amazon Q interactive session (logged to $logfile)"
    q chat 2>&1 | tee "$logfile"
}

# Show Amazon Q help
q-help() {
    cat <<EOF
Amazon Q CLI Commands:
  q-ask "question"     - Ask Amazon Q with logged response
  q-translate "text"   - Convert natural language to shell command
  q-interactive        - Start interactive Q session with logging
  q chat              - Native Amazon Q chat
  q doctor            - Troubleshoot Amazon Q issues
  q logout            - Log out from Amazon Q
  q --help            - Show all Amazon Q commands
EOF
}

# Export functions
export -f q-ask
export -f q-translate
export -f q-interactive
export -f q-help
```

Update `startup.sh` to source this script:

```bash
# Set up Amazon Q CLI logging commands (after Claude commands setup)
if [ "${AMAZONQ_AUTH_OK}" = "true" ]; then
    log "Setting up Amazon Q CLI logging commands"
    if [ -f /app/scripts/amazon-q-commands.sh ]; then
        source /app/scripts/amazon-q-commands.sh
        log "✅ Amazon Q logging commands available: q-ask, q-translate, q-interactive"
    fi
fi
```

---

## 5. Rust TUI Integration

### 5.1 Configuration Changes

**File**: `src/config/mod.rs`

Add AI provider configuration:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    // ... existing fields ...

    #[serde(default)]
    pub ai_config: AIConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIConfig {
    /// Default AI provider: claude, amazonq, or both
    #[serde(default = "default_ai_provider")]
    pub default_provider: AIProvider,

    /// Amazon Q configuration
    #[serde(default)]
    pub amazon_q: AmazonQConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AIProvider {
    Claude,
    AmazonQ,
    Both,  // Allow user to choose at session creation
}

fn default_ai_provider() -> AIProvider {
    AIProvider::Claude
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmazonQConfig {
    /// License type: free or pro
    #[serde(default = "default_license")]
    pub license: String,

    /// IAM Identity Center URL (for pro)
    pub identity_provider: Option<String>,

    /// AWS region
    #[serde(default = "default_region")]
    pub region: String,

    /// Enable Amazon Q in containers
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_license() -> String {
    "free".to_string()
}

fn default_region() -> String {
    "us-east-1".to_string()
}

fn default_true() -> bool {
    true
}
```

### 5.2 Session Model Changes

**File**: `src/models/session.rs`

Add AI provider to session:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    // ... existing fields ...

    /// AI provider for this session
    #[serde(default)]
    pub ai_provider: AIProvider,
}

impl Session {
    pub fn new(name: String, workspace_path: String, branch_name: String) -> Self {
        Self {
            // ... existing initialization ...
            ai_provider: AIProvider::Claude,  // Default
        }
    }
}
```

### 5.3 New Session Dialog Enhancement

**File**: `src/components/new_session.rs`

Add AI provider selection:

```rust
pub struct NewSessionDialog {
    // ... existing fields ...
    ai_provider_index: usize,
    available_ai_providers: Vec<AIProvider>,
}

impl NewSessionDialog {
    pub fn new(workspace_path: String, available_templates: Vec<ContainerTemplate>) -> Self {
        Self {
            // ... existing initialization ...
            ai_provider_index: 0,
            available_ai_providers: vec![AIProvider::Claude, AIProvider::AmazonQ],
        }
    }

    fn render_ai_provider_selection(&self, area: Rect, buf: &mut Buffer) {
        let providers: Vec<ListItem> = self.available_ai_providers
            .iter()
            .enumerate()
            .map(|(i, provider)| {
                let style = if i == self.ai_provider_index {
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let provider_name = match provider {
                    AIProvider::Claude => "Claude AI (Anthropic)",
                    AIProvider::AmazonQ => "Amazon Q Developer (AWS)",
                    AIProvider::Both => "Both (Choose per task)",
                };

                ListItem::new(provider_name).style(style)
            })
            .collect();

        let list = List::new(providers)
            .block(Block::default()
                .title("AI Provider")
                .borders(Borders::ALL));

        Widget::render(list, area, buf);
    }
}
```

### 5.4 Session List Display

**File**: `src/components/session_list.rs`

Show AI provider in session list:

```rust
fn render_session_item(&self, session: &Session, is_selected: bool) -> Line {
    // ... existing code ...

    // Add AI provider indicator
    let ai_indicator = match session.ai_provider {
        AIProvider::Claude => Span::styled("🤖", Style::default().fg(Color::Blue)),
        AIProvider::AmazonQ => Span::styled("🔧", Style::default().fg(Color::Yellow)),
        AIProvider::Both => Span::styled("🔀", Style::default().fg(Color::Cyan)),
    };

    // ... build line with ai_indicator ...
}
```

### 5.5 Container Launch Integration

**File**: `src/docker/session_lifecycle.rs`

Pass AI provider to container:

```rust
pub async fn create_session_container(
    &self,
    session: &Session,
    template: &ContainerTemplate,
) -> Result<String> {
    // ... existing code ...

    // Add AI provider environment variable
    let mut env_vars = config_env;
    env_vars.push(format!("AI_PROVIDER={}", session.ai_provider.to_string()));

    // Add Amazon Q specific env vars if needed
    if session.ai_provider == AIProvider::AmazonQ || session.ai_provider == AIProvider::Both {
        let amazon_q_config = &self.config.ai_config.amazon_q;
        env_vars.push(format!("AMAZON_Q_LICENSE={}", amazon_q_config.license));
        env_vars.push(format!("AMAZON_Q_REGION={}", amazon_q_config.region));

        if let Some(idp) = &amazon_q_config.identity_provider {
            env_vars.push(format!("AMAZON_Q_IDP={}", idp));
        }
    }

    // ... rest of container creation ...
}
```

---

## 6. Configuration Examples

### 6.1 Global Configuration

**File**: `~/.claude-in-a-box/config/config.toml`

```toml
version = "0.1.0"
default_container_template = "claude-dev"

# AI Configuration
[ai_config]
default_provider = "claude"  # Options: "claude", "amazonq", "both"

# Amazon Q Configuration
[ai_config.amazon_q]
enabled = true
license = "free"  # Options: "free", "pro"
region = "us-east-1"
# identity_provider = "https://my-company.awsapps.com/start"  # For pro

# Container templates
[container_templates.claude-dev]
name = "claude-dev"
description = "Claude and Amazon Q development environment"

[container_templates.claude-dev.config]
image_source = { type = "ClaudeDocker", base_image = "node:20-slim" }
working_dir = "/workspace"
memory_limit = 4096
cpu_limit = 2.0
mount_ssh = true
mount_git_config = true

# ... rest of config ...
```

### 6.2 Project Configuration

**File**: `.claude-in-a-box/project.toml`

```toml
container_template = "claude-dev"
mount_claude_config = true

# Override AI provider for this project
[ai_config]
default_provider = "amazonq"  # Use Amazon Q for this project

[ai_config.amazon_q]
license = "pro"
identity_provider = "https://my-company.awsapps.com/start"
region = "us-west-2"

[environment]
NODE_ENV = "development"
BOSS_MODE_PROVIDER = "amazonq"  # Use Amazon Q in boss mode
```

### 6.3 Session-Specific Override

```bash
# Launch session with specific AI provider
AI_PROVIDER=amazonq cargo run

# Boss mode with Amazon Q
CLAUDE_BOX_MODE=boss \
BOSS_MODE_PROVIDER=amazonq \
CLAUDE_BOX_PROMPT="Implement user authentication" \
./docker/claude-dev/claude-dev.sh
```

---

## 7. Boss Mode Integration

### 7.1 Boss Mode Flow for Amazon Q

1. **Session Creation**: User creates session with Amazon Q as provider
2. **Container Launch**: TUI launches container with `CLAUDE_BOX_MODE=boss` and `AI_PROVIDER=amazonq`
3. **Startup Script**: Detects boss mode + Amazon Q provider
4. **Prompt Enhancement**: Appends project rules to user prompt (same as Claude)
5. **Execution**: Runs `q chat --no-interactive --trust-all-tools "enhanced prompt"`
6. **Output Streaming**: Captures output to logs for TUI display

### 7.2 Boss Mode Command

**Amazon Q Boss Mode**:
```bash
q chat --no-interactive --trust-all-tools "${ENHANCED_PROMPT}"
```

**Options**:
- `--no-interactive`: Non-interactive mode (no prompts)
- `--trust-all-tools`: Automatically approve tool usage
- Prompt includes original + boss mode guidelines

### 7.3 Output Handling

Amazon Q output format differs from Claude:
- **Claude**: Supports `--output-format stream-json`
- **Amazon Q**: Plain text output (no structured JSON)

**Solution**: Create output parser for Amazon Q in startup script:

```bash
# Amazon Q output capture with formatting
q chat --no-interactive --trust-all-tools "${ENHANCED_PROMPT}" 2>&1 | \
    while IFS= read -r line; do
        # Timestamp and format output for TUI
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] $line"
    done
```

---

## 8. Authentication Management

### 8.1 Authentication Setup Flow

#### 8.1.1 Initial Setup (Host Machine)

```bash
# Install Amazon Q on host (if not already)
# macOS
brew install --cask amazon-q

# Linux
curl -fsSL "https://desktop-release.q.us-east-1.amazonaws.com/latest/amazon-q.deb" -o amazon-q.deb
sudo dpkg -i amazon-q.deb

# Authenticate on host
q login --license free  # or pro with --identity-provider

# Setup for claude-in-a-box
./scripts/setup-amazon-q.sh
```

#### 8.1.2 TUI-Integrated Authentication

Add authentication wizard to TUI (new component):

**File**: `src/components/amazon_q_auth.rs`

```rust
pub struct AmazonQAuthDialog {
    license_type: String,  // "free" or "pro"
    idp_url: String,       // For pro
    region: String,
    status: AuthStatus,
}

impl AmazonQAuthDialog {
    pub async fn authenticate(&mut self) -> Result<()> {
        // Launch auth container with device flow
        // Monitor authentication progress
        // Copy credentials to shared location
    }
}
```

#### 8.1.3 Container Authentication

**Options**:

1. **Pre-authenticated (Recommended)**: Mount `~/.aws/q` from host
2. **Device Flow**: Authenticate in container using `--use-device-flow`
3. **Shared Credentials**: Use shared auth directory

**Dockerfile Volume Mounts**:
```bash
-v "$HOME/.aws/q:/home/claude-user/.aws/q:rw"
```

### 8.2 Authentication Verification

**Health Check Script** (`scripts/check-amazon-q-auth.sh`):

```bash
#!/bin/bash
# Check Amazon Q authentication status

if ! command -v q &> /dev/null; then
    echo "Amazon Q CLI not installed"
    exit 1
fi

if q doctor &> /dev/null; then
    echo "✅ Amazon Q authenticated"
    q doctor
    exit 0
else
    echo "❌ Amazon Q not authenticated"
    echo "Run: q login --license free"
    exit 1
fi
```

### 8.3 Multi-Session Authentication Sharing

All sessions share the same Amazon Q authentication via:
1. Host authentication mounted to containers
2. Shared `~/.claude-in-a-box/amazon-q` directory
3. Container volume mounts reference shared location

**No per-session authentication needed** - authenticate once, use everywhere.

---

## 9. Implementation Phases

### Phase 1: Docker Integration (Week 1)
- [ ] Modify Dockerfile to install Amazon Q CLI
- [ ] Add Amazon Q configuration directory setup
- [ ] Create volume mounts for authentication
- [ ] Test basic Amazon Q installation in container
- [ ] Create setup script for host authentication

### Phase 2: Startup Script Integration (Week 1-2)
- [ ] Add Amazon Q authentication detection
- [ ] Implement AI provider selection logic
- [ ] Modify boss mode for Amazon Q support
- [ ] Add Amazon Q helper commands script
- [ ] Update interactive mode messaging

### Phase 3: Rust Configuration (Week 2)
- [ ] Add AI provider enums and structs
- [ ] Update AppConfig with AI configuration
- [ ] Add Amazon Q specific config options
- [ ] Implement configuration loading/merging
- [ ] Add validation for AI provider settings

### Phase 4: TUI Components (Week 2-3)
- [ ] Modify Session model to include AI provider
- [ ] Update new session dialog with provider selection
- [ ] Add AI provider indicator to session list
- [ ] Create Amazon Q authentication dialog component
- [ ] Update session details panel

### Phase 5: Container Lifecycle (Week 3)
- [ ] Update session_lifecycle.rs for AI provider
- [ ] Pass AI provider environment variables
- [ ] Handle Amazon Q specific volume mounts
- [ ] Update container builder with Amazon Q
- [ ] Test container creation with both providers

### Phase 6: Boss Mode Testing (Week 3-4)
- [ ] Test boss mode with Claude (baseline)
- [ ] Test boss mode with Amazon Q
- [ ] Compare output formats and parsing
- [ ] Implement output normalization if needed
- [ ] Create boss mode comparison tests

### Phase 7: Documentation and Polish (Week 4)
- [ ] Update README with Amazon Q integration
- [ ] Create Amazon Q setup guide
- [ ] Add troubleshooting section for Amazon Q
- [ ] Create configuration examples
- [ ] Add inline code documentation

### Phase 8: Testing and Validation (Week 4-5)
- [ ] Unit tests for AI provider logic
- [ ] Integration tests with Docker
- [ ] Boss mode automated tests
- [ ] Authentication flow tests
- [ ] Multi-session concurrency tests

---

## 10. Testing Strategy

### 10.1 Unit Tests

**File**: `tests/ai_provider_tests.rs`

```rust
#[cfg(test)]
mod ai_provider_tests {
    use super::*;

    #[test]
    fn test_ai_provider_from_string() {
        assert_eq!(AIProvider::from_str("claude"), AIProvider::Claude);
        assert_eq!(AIProvider::from_str("amazonq"), AIProvider::AmazonQ);
    }

    #[test]
    fn test_ai_provider_env_vars() {
        let provider = AIProvider::AmazonQ;
        let env_vars = provider.get_env_vars();
        assert!(env_vars.contains(&"AI_PROVIDER=amazonq".to_string()));
    }
}
```

### 10.2 Integration Tests

**Test Scenarios**:

1. **Authentication Tests**
   - Host authentication sync
   - Container authentication detection
   - Multi-session authentication sharing

2. **Boss Mode Tests**
   - Claude boss mode execution
   - Amazon Q boss mode execution
   - Prompt enhancement verification

3. **Session Creation Tests**
   - Create session with Claude
   - Create session with Amazon Q
   - Create session with both providers

4. **Configuration Tests**
   - Load default AI provider
   - Override with project config
   - Environment variable overrides

### 10.3 Manual Testing Checklist

- [ ] Install Amazon Q on host
- [ ] Authenticate with Builder ID
- [ ] Authenticate with IAM Identity Center (if available)
- [ ] Create session with Claude
- [ ] Create session with Amazon Q
- [ ] Test boss mode with Claude
- [ ] Test boss mode with Amazon Q
- [ ] Test interactive mode with both
- [ ] Test authentication sharing across sessions
- [ ] Test configuration overrides
- [ ] Test error handling (no auth, invalid provider)

---

## 11. Known Limitations and Workarounds

### 11.1 Headless Authentication

**Issue**: Amazon Q CLI has limited support for fully non-interactive authentication in headless environments.

**Workarounds**:
1. **Pre-authenticate on Host**: Authenticate on host machine, mount credentials
2. **Device Flow**: Use `--use-device-flow` for manual authentication
3. **Persistent Credentials**: Share credentials directory across sessions

### 11.2 Output Format

**Issue**: Amazon Q doesn't support structured JSON output like Claude's `--output-format stream-json`.

**Workarounds**:
1. **Plain Text Parsing**: Parse Amazon Q output as plain text
2. **Timestamped Logging**: Add timestamps to output for TUI display
3. **Unified Output Handler**: Create output formatter to normalize both providers

### 11.3 Non-Interactive Mode

**Issue**: Amazon Q's `--no-interactive` mode may still prompt for confirmations in some cases.

**Workarounds**:
1. **Trust All Tools Flag**: Use `--trust-all-tools` to reduce prompts
2. **Pre-configured Settings**: Set default options in Amazon Q config
3. **Expect Script**: Wrap `q chat` with expect for full automation (if needed)

### 11.4 License Management

**Issue**: Pro license requires organization-level setup with IAM Identity Center.

**Workarounds**:
1. **Default to Free**: Use Builder ID (free) by default
2. **Project-Level Override**: Allow pro license per-project in config
3. **Environment Variables**: Support license type via environment variables

---

## 12. Configuration Reference

### 12.1 Environment Variables

| Variable | Description | Default | Example |
|----------|-------------|---------|---------|
| `AI_PROVIDER` | AI provider to use | `claude` | `amazonq`, `claude`, `both` |
| `BOSS_MODE_PROVIDER` | AI provider for boss mode | Same as `AI_PROVIDER` | `amazonq`, `claude` |
| `AMAZON_Q_LICENSE` | License type | `free` | `free`, `pro` |
| `AMAZON_Q_IDP` | IAM Identity Center URL | - | `https://company.awsapps.com/start` |
| `AMAZON_Q_REGION` | AWS region | `us-east-1` | `us-west-2`, `eu-west-1` |

### 12.2 Configuration Files

#### Global Config: `~/.claude-in-a-box/config/config.toml`

```toml
[ai_config]
default_provider = "claude"

[ai_config.amazon_q]
enabled = true
license = "free"
region = "us-east-1"
```

#### Project Config: `.claude-in-a-box/project.toml`

```toml
[ai_config]
default_provider = "amazonq"

[environment]
BOSS_MODE_PROVIDER = "amazonq"
AMAZON_Q_LICENSE = "pro"
```

### 12.3 Volume Mounts

| Host Path | Container Path | Purpose |
|-----------|----------------|---------|
| `~/.aws/q` | `/home/claude-user/.aws/q` | Amazon Q auth and config |
| `~/.claude-in-a-box/amazon-q` | `/home/claude-user/.aws/q` | Shared auth (alternative) |

---

## 13. Security Considerations

### 13.1 Authentication Storage

- **Amazon Q Credentials**: Stored in system keychain + `~/.aws/q`
- **Mount Security**: Read-write mounts only for user-owned directories
- **Isolation**: Each container has isolated AWS CLI credentials (unless mounted)

### 13.2 License Compliance

- **Builder ID**: Free tier, per-user, bound to AWS account
- **Pro License**: Organization-level, requires IAM Identity Center
- **Validation**: Amazon Q CLI validates license on each use

### 13.3 Best Practices

1. **Don't Commit Credentials**: Add `~/.aws/q` to `.gitignore`
2. **Use IAM Roles**: For EC2/ECS deployments, use IAM roles instead of keys
3. **Rotate Credentials**: Follow AWS security best practices
4. **Audit Usage**: Monitor Amazon Q API usage in AWS Console

---

## 14. Migration Guide

### 14.1 For Existing Users

**No Breaking Changes**: Claude-in-a-box will continue to work with Claude CLI by default.

**Optional Migration Steps**:

1. **Install Amazon Q** (optional):
   ```bash
   brew install --cask amazon-q
   q login --license free
   ```

2. **Update Configuration** (optional):
   ```toml
   [ai_config]
   default_provider = "both"  # Allow choosing at session creation
   ```

3. **Try Amazon Q**:
   - Create new session
   - Select "Amazon Q" as provider
   - Test in interactive or boss mode

### 14.2 For New Users

New users can choose their preferred AI provider during initial setup:

```bash
# First run
cargo run

# Follow setup wizard:
# 1. Choose AI provider: Claude, Amazon Q, or Both
# 2. Authenticate with selected provider(s)
# 3. Create first session
```

---

## 15. Future Enhancements

### 15.1 Short Term (Next 3 months)

1. **AI Provider Comparison View**: Side-by-side comparison of responses
2. **Automatic Provider Selection**: Choose provider based on task type
3. **Hybrid Mode**: Use both providers and combine responses
4. **Cost Tracking**: Monitor API usage and costs

### 15.2 Long Term (6+ months)

1. **Additional Providers**: Gemini, GPT-4, local LLMs
2. **Provider Routing**: Intelligent routing based on task complexity
3. **Multi-Agent Workflows**: Coordinate multiple AI agents
4. **Custom Provider Plugins**: Allow users to add custom providers

---

## 16. Resources

### 16.1 Documentation

- [Amazon Q Developer CLI Official Docs](https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/command-line.html)
- [Amazon Q CLI Command Reference](https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/command-line-reference.html)
- [Amazon Q GitHub Repository](https://github.com/aws/amazon-q-developer-cli)

### 16.2 Installation Resources

- [Installation Guide](https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/command-line-installing.html)
- [Linux Installation Guide (DEV.to)](https://dev.to/aws/the-essential-guide-to-installing-amazon-q-developer-cli-on-linux-headless-and-desktop-3bo7)
- [Docker Container Setup](https://community.aws/content/2uZYCp6BNJJgBaRnw3Nie6i8r0l/putting-amazon-q-developer-in-a-docker-container)

### 16.3 Pricing

- **Builder ID (Free)**: Free forever, basic features
- **Pro License**: $19/month per user, advanced features, enterprise support
- [Pricing Details](https://aws.amazon.com/q/developer/pricing/)

---

## 17. Success Metrics

### 17.1 Technical Metrics

- [ ] Amazon Q CLI successfully installs in Docker container
- [ ] Authentication works in containerized environment
- [ ] Boss mode executes correctly with Amazon Q
- [ ] Session creation with Amazon Q succeeds
- [ ] Multi-session authentication sharing works
- [ ] Configuration overrides function properly

### 17.2 User Experience Metrics

- [ ] Users can easily choose between Claude and Amazon Q
- [ ] Setup process is straightforward (< 5 minutes)
- [ ] Boss mode produces equivalent quality outputs
- [ ] Interactive mode works smoothly with both providers
- [ ] Documentation is clear and comprehensive

### 17.3 Performance Metrics

- [ ] Container startup time: < 10 seconds
- [ ] Boss mode response time: comparable to Claude
- [ ] Authentication verification: < 2 seconds
- [ ] TUI remains responsive with both providers

---

## 18. Open Questions

### 18.1 Technical Questions

1. **Output Streaming**: How to handle Amazon Q's text output vs Claude's JSON streaming?
   - **Proposed**: Parse text output, add structured logging wrapper

2. **Tool Execution**: Does Amazon Q support tool/command execution like Claude MCP?
   - **Research**: Check Amazon Q documentation for tool support

3. **Resource Usage**: Memory and CPU usage comparison between providers?
   - **Action**: Benchmark both providers in containers

### 18.2 Product Questions

1. **Default Provider**: Should we default to Claude or make it required choice?
   - **Proposed**: Default to Claude, allow easy switching

2. **License Warning**: How to communicate Pro license requirements?
   - **Proposed**: Show license status in TUI, warn if Pro features needed

3. **Feature Parity**: Which features work with both providers?
   - **Action**: Create feature compatibility matrix

---

## 19. Rollout Plan

### 19.1 Alpha Release (Internal Testing)

**Duration**: 1 week
**Audience**: Development team
**Goals**:
- Validate Docker integration
- Test authentication flows
- Identify critical bugs

### 19.2 Beta Release (Early Adopters)

**Duration**: 2-3 weeks
**Audience**: Selected users with Amazon Q access
**Goals**:
- Gather feedback on UX
- Test with real-world projects
- Refine configuration options

### 19.3 General Availability

**Duration**: Ongoing
**Audience**: All users
**Goals**:
- Stable multi-provider support
- Comprehensive documentation
- Community feedback integration

---

## 20. Conclusion

This integration plan provides a comprehensive roadmap for adding Amazon Q CLI to claude-in-a-box while maintaining backward compatibility with Claude CLI. The phased approach allows for incremental development and testing, ensuring a robust and user-friendly implementation.

Key benefits of this integration:
1. **Choice**: Users can select their preferred AI provider
2. **Flexibility**: Per-project and per-session provider selection
3. **Compatibility**: Works with existing Claude workflows
4. **Cost Options**: Free (Builder ID) and Pro (IAM) tiers available
5. **Future-Proof**: Architecture supports additional providers

By following this plan, claude-in-a-box will become a truly multi-provider AI development environment, giving users the best of both worlds: Claude's powerful reasoning and Amazon Q's AWS-native integration.

---

**Document Version**: 1.0
**Last Updated**: 2025-10-29
**Author**: Claude AI
**Status**: Draft for Review
