# Claude-in-a-Box

A terminal-based development environment manager that creates isolated Docker containers running Claude Code with dedicated git worktrees. Quickly spin up, manage, and switch between multiple AI-assisted coding sessions across different projects.

![Claude-in-a-Box Demo](docs/demo.gif)

## ✨ Features

- **Isolated Sessions**: Each coding session runs in its own Docker container with dedicated git worktree
- **Multi-Project Support**: Manage sessions across multiple git repositories
- **Terminal UI**: Intuitive split-pane interface with vim-style navigation
- **Real-time Logs**: Stream container logs and monitor session status
- **Quick Switching**: Jump between sessions without losing context
- **Session Persistence**: Resume sessions after application restart

## 🚀 Quick Start

### Prerequisites

- **Rust** (latest stable) - [Install Rust](https://rustup.rs/)
- **Docker** - [Install Docker](https://docs.docker.com/get-docker/)
- **Git** - For worktree management
- **Just** (optional) - [Install Just](https://github.com/casey/just) for convenient commands

### Installation

```bash
# Clone the repository
git clone https://github.com/your-org/claude-box
cd claude-box

# Build the project
cargo build --release

# Run the application
cargo run
```

### Using Just (Recommended)

```bash
# Install development dependencies
just setup

# Check everything (format, lint, test)
just check

# Run the application
just run

# Run tests
just test
```

## 🎮 Usage

### Basic Navigation

```
j/↓        Move down in session list
k/↑        Move up in session list  
h/←        Previous workspace
l/→        Next workspace
g          Go to top
G          Go to bottom
```

### Session Management

```
n          Create new session
a          Attach to session (interactive terminal)
s          Start/Stop session
d          Delete session
```

### Interface Controls

```
?          Toggle help overlay
Tab        Switch between views
q/Esc      Quit application
Ctrl+C     Force quit
```

### Interface Layout

```
┌─ Claude-in-a-Box ──────────────────────────────────────────┐
│ Workspaces         │ Session: project1/fix-auth            │
│                    │ Status: ● Running                      │
│ ▼ project1         │ Branch: claude/fix-auth                │
│   ● fix-auth       │ Changes: +42 -13                       │
│   ⏸ add-feature    ├────────────────────────────────────────│
│   ✗ debug-issue    │ Logs                                   │
│                    │ ─────────────────────────────────────  │
│ ▶ project2         │ Starting Claude Code environment...    │
│   ● refactor-api   │ Loading MCP servers...                 │
│                    │ Ready! Attached to container.          │
├────────────────────┴────────────────────────────────────────┤
│ [n]ew [a]ttach [s]tart/stop [d]elete [w]orkspace [q]uit    │
└─────────────────────────────────────────────────────────────┘
```

## 🏗️ Architecture

### Project Structure

```
src/
├── app/           # Application state and event handling
├── components/    # TUI components (session list, logs, help)
├── docker/        # Docker container management
├── git/           # Git worktree operations
├── models/        # Data structures (Session, Workspace, etc.)
└── utils/         # Utility functions
```

### Development Phases

- **✅ Phase 1**: UI Foundation - Complete TUI with navigation
- **✅ Phase 2**: Data Models - Session and workspace management
- **🔄 Phase 3**: Git Integration - Worktree creation and management
- **📋 Phase 4**: Docker Integration - Container lifecycle
- **📋 Phase 5**: Interactive Features - TTY attachment
- **📋 Phase 6**: Persistence & Polish - State management

## 🧪 Development

### Requirements

- Rust 1.70+
- Docker daemon running
- Git repositories for testing

### Development Workflow

```bash
# Format code
just fmt

# Run linter
just lint

# Run tests
just test

# Run everything
just check

# Watch for changes
just watch
```

### Testing

```bash
# Run unit tests
cargo test

# Run with output
just test-verbose

# Run specific test
cargo test test_session_model
```

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Cross-compilation (example)
cargo build --target x86_64-unknown-linux-gnu
```

## 📁 Configuration

### Session Storage

Sessions are stored in `~/.claude-box/`:

```
~/.claude-box/
├── sessions.json    # Session metadata
├── config.toml      # Application configuration
└── worktrees/       # Git worktrees
    └── {session-id}/
```

### Environment Files

Claude-in-a-Box automatically copies environment files into containers:
- `.env`
- `.env.local`
- Custom environment files (configurable)

## 🐳 Docker Integration

### Container Requirements

- Based on official Claude Code Docker image
- Workspace mounted as volume at `/workspace`
- Environment variables from `.env` files
- Non-privileged execution for security

### Container Lifecycle

1. **Create**: New container from Claude Code image
2. **Mount**: Workspace as volume, copy env files
3. **Start**: Launch Claude Code with MCP servers
4. **Attach**: Interactive TTY for user interaction
5. **Detach**: Background execution while preserving state
6. **Stop**: Graceful shutdown with state preservation

## 🔧 Troubleshooting

### Common Issues

**Application won't start**
```bash
# Check Rust installation
rustc --version

# Check Docker daemon
docker ps

# Run with debug logs
RUST_LOG=debug cargo run
```

**Docker container issues**
```bash
# Check Docker connectivity
docker version

# List containers
docker ps -a

# View container logs
docker logs <container-id>
```

**Git worktree conflicts**
```bash
# List existing worktrees
git worktree list

# Remove stale worktrees
git worktree prune
```

### Debug Mode

Enable detailed logging:

```bash
export RUST_LOG=debug
cargo run
```

### Performance Issues

- Limit concurrent sessions (default: 10)
- Monitor container resource usage
- Check disk space for worktrees

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md).

### Getting Started

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/amazing-feature`
3. Make your changes
4. Run tests: `just check`
5. Commit: `git commit -m 'Add amazing feature'`
6. Push: `git push origin feature/amazing-feature`
7. Open a Pull Request

### Development Guidelines

- Follow clean code principles
- Write tests for new functionality
- Use conventional commit messages
- Update documentation as needed

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [Ratatui](https://github.com/ratatui-org/ratatui) - Terminal UI framework
- [Bollard](https://github.com/fussybeaver/bollard) - Docker API client
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) - AI pair programming
- [Crossterm](https://github.com/crossterm-rs/crossterm) - Cross-platform terminal library

## 🔗 Related Projects

- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) - Official Claude CLI
- [Docker](https://docker.com) - Containerization platform
- [Git Worktree](https://git-scm.com/docs/git-worktree) - Multiple working trees

---

**Made with ❤️ by the Claude-in-a-Box team**

For questions, issues, or feature requests, please [open an issue](https://github.com/your-org/claude-box/issues).