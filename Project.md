# Claude-in-a-Box Development Project

## Project Overview

Claude-in-a-Box is a terminal-based development environment manager that provides isolated Docker containers with integrated Claude AI tools and MCP (Model Context Protocol) servers for enhanced development workflows.

## Development Progress

### ✅ Phase 1: UI Foundation (COMPLETED)

- [x] Terminal User Interface (TUI) with Ratatui
- [x] Basic application structure with components
- [x] Event handling system
- [x] Layout management
- [x] Session state management

### ✅ Phase 2: Core Models (COMPLETED)

- [x] Session data models
- [x] Workspace management models
- [x] Container lifecycle models
- [x] Application state management

### ✅ Phase 3: Git Integration (COMPLETED)

- [x] Workspace detection and scanning
- [x] Git worktree operations with hybrid path structure
- [x] Branch isolation for development sessions
- [x] Git status integration
- [x] Cross-platform symlink support

### ✅ Phase 4: Docker Integration (COMPLETED)

- [x] Docker container management using Bollard
- [x] Session lifecycle coordination (worktrees + containers)
- [x] Container status monitoring and logging
- [x] Volume mounting for workspace isolation
- [x] Port mapping with dynamic assignment
- [x] Resource limits (CPU, memory)

### 🆕 Phase 4.5: Configuration & Templates (COMPLETED)

- [x] **JSON/TOML Configuration System**
  - Global config: `~/.claude-in-a-box/config/config.toml`
  - Project config: `.claude-in-a-box/project.toml`
  - Configuration merging and precedence

- [x] **Container Templates**
  - `claude-dev`: Full Claude development environment based on claude-docker
  - `node`: Node.js development environment
  - `python`: Python development environment
  - `rust`: Rust development environment
  - Template-based container configuration

- [x] **Claude-Docker Integration**
  - Reused claude-docker Dockerfile and setup scripts
  - MCP server installation and configuration
  - Authentication handling (Claude API keys)
  - Environment variable management

- [x] **MCP Initialization Strategy**
  - **Per-Container**: MCP servers installed in each container
  - **Central Mount**: Mount host ~/.claude directory
  - **Hybrid**: Install in container + mount host config (default)
  - Automatic MCP server discovery and configuration
  - Environment variable validation for optional servers

### 🔄 Phase 5: Interactive Features (PENDING)

- [ ] Container template selection UI
- [ ] Real-time container status monitoring
- [ ] Log viewer and streaming
- [ ] Interactive session management
- [ ] MCP server status and configuration UI

### 🔄 Phase 6: Persistence & Polish (PENDING)

- [ ] Session persistence across restarts
- [ ] Configuration file generation UI
- [ ] Error handling and recovery
- [ ] Performance optimizations
- [ ] Documentation generation

## Architecture

### Core Components

1. **Configuration System** (`src/config/`)
   - `AppConfig`: Global application configuration
   - `ProjectConfig`: Project-specific overrides
   - `ContainerTemplate`: Pre-configured development environments
   - `McpServerConfig`: MCP server definitions and installation

2. **Docker Integration** (`src/docker/`)
   - `ContainerManager`: Low-level Docker operations
   - `SessionLifecycleManager`: High-level session orchestration
   - `ImageBuilder`: Docker image building for custom templates
   - `SessionContainer`: Container models and status tracking

3. **Git Integration** (`src/git/`)
   - `WorkspaceScanner`: Git repository discovery
   - `WorktreeManager`: Git worktree operations
   - Hybrid path structure: human-readable + session-based

4. **MCP Integration** (`src/config/mcp*.rs`)
   - `McpInitializer`: MCP server initialization strategies
   - `McpServerConfig`: Server definitions (Serena, Context7, Twilio)
   - Configuration merging between host and container

### Default Container Template (claude-dev)

Based on the existing claude-docker implementation:

- **Base Image**: `node:20-slim`
- **User**: `claude-user` (UID/GID matched to host)
- **AI Tools**:
  - `@anthropic-ai/claude-code` (Claude CLI)
  - `@google/gemini-cli` (Gemini CLI)
- **MCP Servers**:
  - **Serena**: AI coding agent toolkit
  - **Context7**: Library documentation and examples
  - **Twilio**: SMS notifications (optional, requires env vars)
- **Development Tools**: Git, build-essential, Python 3, Node.js
- **Workspace**: Mounted at `/workspace`
- **Authentication**: Host ~/.claude.json or ANTHROPIC_API_KEY

### Configuration Examples

#### Global Configuration (`~/.config/claude-box/config.toml`)

```toml
version = "0.1.0"
default_container_template = "claude-dev"

[workspace_defaults]
branch_prefix = "claude/"
auto_detect = true
exclude_paths = ["node_modules", ".git", "target"]

[ui_preferences]
theme = "dark"
show_container_status = true
show_git_status = true

# MCP servers are loaded from defaults with environment validation
```

#### Project Configuration (`.claude-in-a-box/project.toml`)

```toml
container_template = "claude-dev"
mount_claude_config = true

[environment]
NODE_ENV = "development"
DEBUG = "myapp:*"

[[additional_mounts]]
host_path = "~/.ssh"
container_path = "/home/claude-user/.ssh"
read_only = true

[container_config]
memory_limit = 4096  # MB
cpu_limit = 2.0      # CPUs
```

## Development Environment Setup

### Prerequisites

- Docker installed and running
- Rust toolchain (for building)
- Git

### Building

```bash
# Clone the repository
git clone <repository-url>
cd claude-docker

# Build the application
cargo build --release

# Run integration tests (requires Docker)
cargo test --ignored
```

### Container Templates

Templates are built automatically when needed:

```bash
# The claude-dev template will be built from docker/claude-dev/Dockerfile
# when first used, including:
# - MCP server installation
# - Authentication setup
# - Development tool configuration
```

## Usage Workflow

1. **Workspace Detection**: Auto-scan for Git repositories
2. **Template Selection**: Choose or auto-detect container template
3. **Session Creation**:
   - Create isolated git worktree
   - Build/pull container image
   - Initialize MCP servers
   - Mount workspace and configuration
4. **Development**:
   - Full Claude CLI with MCP servers
   - Isolated development environment
   - Git operations in dedicated worktree
5. **Cleanup**: Automatic worktree and container cleanup

## Current Status: Phase 4.5 Complete ✅

The project now has a complete configuration system, container templates based on claude-docker, and flexible MCP initialization strategies. Users can:

- Use pre-configured templates or specify custom containers
- Configure MCP servers with multiple initialization strategies
- Mount host authentication and configuration as needed
- Override settings per-project with `.claude-in-a-box/project.toml`

Ready for Phase 5: Interactive Features development.

## Next Steps

1. **Container Template Selection UI**: Allow users to choose templates interactively
2. **Real-time Monitoring**: Show container status, logs, and resource usage
3. **MCP Server Management**: Enable/disable servers, view status, configure on the fly
4. **Session Persistence**: Save and restore development sessions
5. **Error Recovery**: Handle Docker/Git failures gracefully

The foundation is solid - now we build the interactive experience! 🚀
