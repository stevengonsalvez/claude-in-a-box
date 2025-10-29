# Amazon Q Integration - Quick Start Guide

This is a condensed guide for integrating Amazon Q CLI into claude-in-a-box. For the comprehensive plan, see [AMAZON_Q_INTEGRATION_PLAN.md](./AMAZON_Q_INTEGRATION_PLAN.md).

## What is Amazon Q CLI?

Amazon Q Developer CLI is AWS's AI-powered coding assistant that provides:
- Natural language chat interface (`q chat`)
- Command translation (`q translate`)
- Free tier with AWS Builder ID
- Pro tier ($19/month) with IAM Identity Center

## Key Integration Points

### 1. Docker Integration

**Dockerfile Changes** (`docker/claude-dev/Dockerfile`):

```dockerfile
# Add after AWS CLI installation (~line 78)
RUN curl -fsSL "https://desktop-release.q.us-east-1.amazonaws.com/latest/amazon-q.deb" -o "/tmp/amazon-q.deb" && \
    sudo dpkg -i /tmp/amazon-q.deb || sudo apt-get install -f -y && \
    rm /tmp/amazon-q.deb && \
    q --version || echo "Amazon Q CLI installed"

# Create Amazon Q config directory
RUN mkdir -p /home/claude-user/.aws/q && \
    chown -R claude-user:claude-user /home/claude-user/.aws
```

**Volume Mounts**:
```bash
-v "$HOME/.aws/q:/home/claude-user/.aws/q:rw"
```

### 2. Boss Mode Support

**Startup Script** (`docker/claude-dev/scripts/startup.sh`):

```bash
# Boss mode with Amazon Q
if [ "${BOSS_AI_PROVIDER}" = "amazonq" ]; then
    exec q chat --no-interactive --trust-all-tools "${ENHANCED_PROMPT}"
fi
```

**Environment Variables**:
- `AI_PROVIDER=amazonq` - Use Amazon Q
- `BOSS_MODE_PROVIDER=amazonq` - Amazon Q for boss mode

### 3. Configuration

**Global Config** (`~/.claude-in-a-box/config/config.toml`):

```toml
[ai_config]
default_provider = "claude"  # Options: "claude", "amazonq", "both"

[ai_config.amazon_q]
enabled = true
license = "free"  # Options: "free", "pro"
region = "us-east-1"
# identity_provider = "https://company.awsapps.com/start"  # For pro
```

**Project Config** (`.claude-in-a-box/project.toml`):

```toml
[ai_config]
default_provider = "amazonq"

[environment]
BOSS_MODE_PROVIDER = "amazonq"
```

### 4. Authentication Setup

**On Host**:
```bash
# Install Amazon Q
brew install --cask amazon-q  # macOS
# or
curl -fsSL "https://desktop-release.q.us-east-1.amazonaws.com/latest/amazon-q.deb" -o amazon-q.deb
sudo dpkg -i amazon-q.deb

# Authenticate
q login --license free  # Free tier
# or
q login --license pro --identity-provider https://company.awsapps.com/start --region us-east-1
```

**Sync to claude-in-a-box**:
```bash
mkdir -p ~/.claude-in-a-box/amazon-q
cp -r ~/.aws/q/* ~/.claude-in-a-box/amazon-q/
```

### 5. Usage Examples

**Interactive Mode**:
```bash
# Inside container
q chat                              # Start chat
q chat "How do I list files?"       # Ask question
q translate "show disk usage"       # Translate to command
q doctor                            # Troubleshoot
```

**Boss Mode**:
```bash
# Launch boss mode with Amazon Q
AI_PROVIDER=amazonq \
CLAUDE_BOX_MODE=boss \
CLAUDE_BOX_PROMPT="Implement user authentication" \
./docker/claude-dev/claude-dev.sh
```

## Implementation Checklist

### Phase 1: Docker Setup
- [ ] Modify Dockerfile to install Amazon Q
- [ ] Add volume mounts for authentication
- [ ] Test installation in container
- [ ] Create host setup script

### Phase 2: Startup Script
- [ ] Add Amazon Q authentication detection
- [ ] Implement boss mode support
- [ ] Add helper command scripts
- [ ] Update interactive mode

### Phase 3: Rust Config
- [ ] Add AI provider enums
- [ ] Update AppConfig structure
- [ ] Add Amazon Q config options
- [ ] Implement config loading

### Phase 4: TUI Components
- [ ] Add provider selection to new session dialog
- [ ] Show AI provider in session list
- [ ] Update session model
- [ ] Add provider indicator icons

### Phase 5: Testing
- [ ] Unit tests for AI provider logic
- [ ] Integration tests with Docker
- [ ] Boss mode tests with both providers
- [ ] Authentication flow tests

## Quick Commands Reference

### Amazon Q CLI Commands
| Command | Description |
|---------|-------------|
| `q login` | Authenticate |
| `q chat` | Interactive chat |
| `q chat "prompt"` | Single question |
| `q translate "text"` | Natural language to command |
| `q doctor` | Troubleshoot |
| `q logout` | Log out |

### Environment Variables
| Variable | Values | Description |
|----------|--------|-------------|
| `AI_PROVIDER` | `claude`, `amazonq`, `both` | AI provider selection |
| `BOSS_MODE_PROVIDER` | `claude`, `amazonq` | Boss mode provider |
| `AMAZON_Q_LICENSE` | `free`, `pro` | License type |
| `AMAZON_Q_REGION` | `us-east-1`, etc | AWS region |

## Key Files to Modify

1. **Dockerfile**: `docker/claude-dev/Dockerfile`
   - Add Amazon Q installation
   - Create config directories

2. **Startup Script**: `docker/claude-dev/scripts/startup.sh`
   - Authentication detection
   - Boss mode integration
   - Interactive mode support

3. **Config Module**: `src/config/mod.rs`
   - Add AI provider structs
   - Add Amazon Q config

4. **Session Model**: `src/models/session.rs`
   - Add AI provider field

5. **New Session Dialog**: `src/components/new_session.rs`
   - Add provider selection UI

6. **Session Lifecycle**: `src/docker/session_lifecycle.rs`
   - Pass AI provider to containers

## Testing Scenarios

1. **Authentication**
   - [x] Host authentication works
   - [ ] Container detects authentication
   - [ ] Multi-session sharing

2. **Boss Mode**
   - [ ] Claude boss mode (baseline)
   - [ ] Amazon Q boss mode
   - [ ] Prompt enhancement works

3. **Interactive Mode**
   - [ ] Claude interactive
   - [ ] Amazon Q interactive
   - [ ] Switch between providers

4. **Configuration**
   - [ ] Global config loads
   - [ ] Project config overrides
   - [ ] Environment variables work

## Known Limitations

1. **Headless Auth**: Limited non-interactive auth support
   - **Workaround**: Pre-authenticate on host, mount credentials

2. **Output Format**: No structured JSON output like Claude
   - **Workaround**: Parse plain text, add timestamps

3. **License Management**: Pro requires IAM Identity Center
   - **Workaround**: Default to free tier

## Next Steps

1. Review the [full integration plan](./AMAZON_Q_INTEGRATION_PLAN.md)
2. Set up development environment
3. Start with Phase 1: Docker Integration
4. Follow the implementation checklist
5. Test thoroughly before moving to next phase

## Resources

- [Amazon Q CLI Documentation](https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/command-line.html)
- [GitHub Repository](https://github.com/aws/amazon-q-developer-cli)
- [Pricing](https://aws.amazon.com/q/developer/pricing/)

---

**Status**: Planning Phase
**Next Action**: Begin Phase 1 (Docker Integration)
**Estimated Timeline**: 4-5 weeks for full implementation
