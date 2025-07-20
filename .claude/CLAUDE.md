# Claude-in-a-Box Project Instructions

## Project Overview
Claude-in-a-Box is a Rust terminal-based application that manages isolated Docker containers running Claude Code sessions. This project follows the PRD in `docs/prd.md` and implements a sophisticated TUI using Ratatui.

## Team Member Names
- **Stevie-CLI**: The mastermind behind the Docker orchestration
- **Claude-TUI**: The wizard of terminal user interfaces

## Development Standards

### Code Quality
- **Clean Code**: Follow SOLID principles and Rust best practices
- **Testing**: Maintain comprehensive test coverage (unit, integration, E2E)
- **Documentation**: Every public API must have documentation
- **Formatting**: Use `just fmt` for consistent code formatting
- **Linting**: Use `just lint` to catch issues early

### Build & Test Commands
```bash
# Development workflow
just check         # Format check + lint + test
just fmt           # Format code
just lint          # Run clippy
just test          # Run all tests
just test-verbose  # Run tests with output
just build         # Build project
just run           # Run the application

# Watch mode for development
just watch          # Auto-rebuild on changes
```

### Architecture Guidelines

#### Module Structure
```
src/
├── app/           # Application state and event handling
├── components/    # TUI components (session list, logs, help)
├── docker/        # Docker container management (Phase 4)
├── git/           # Git worktree operations (Phase 3)
├── models/        # Data structures (Session, Workspace, etc.)
└── utils/         # Utility functions
```

#### State Management
- **Single Source of Truth**: AppState holds all application state
- **Immutable Updates**: State changes go through event handlers
- **Clear Separation**: UI components are stateless and receive state as props

#### Error Handling
- Use `anyhow::Result` for error propagation
- Create custom error types using `thiserror` for domain-specific errors
- Never panic in user-facing code - handle all errors gracefully

### Implementation Phases

#### ✅ Phase 1: UI Foundation (COMPLETED)
- Basic TUI skeleton with Ratatui
- Session list component with workspace grouping
- Navigation controls (vim-style)
- Right pane views (logs viewer)
- Help overlay system
- **Status**: All components implemented and tested

#### 🔄 Phase 2: Data Models & State (COMPLETED)
- Session and Workspace models
- Mock data layer for development
- State management with navigation
- **Status**: Core models implemented with full test coverage

#### 🔜 Phase 3: Git Integration (NEXT)
- Workspace detection and validation
- Git worktree creation and management
- Git diff statistics integration
- **Priority**: Start with workspace scanning

#### 🔜 Phase 4: Docker Integration
- Docker client setup with Bollard
- Container lifecycle management
- Real-time log streaming
- **Dependency**: Requires Phase 3 completion

#### 🔜 Phase 5: Interactive Features
- TTY attachment for container interaction
- Environment file management
- Signal handling for clean detach
- **Dependency**: Requires Phase 4 completion

#### 🔜 Phase 6: Persistence & Polish
- Session state persistence to disk
- Configuration management
- Error recovery and validation
- **Dependency**: Requires core functionality completion

### Testing Strategy

#### Test Categories
1. **Unit Tests**: Individual functions and methods
2. **Integration Tests**: Component interactions
3. **End-to-End Tests**: Full application workflows

#### Test Organization
- `tests/test_*_model.rs`: Data model tests
- `tests/test_*_component.rs`: UI component tests
- `tests/test_*_integration.rs`: Integration tests

#### Mock Strategy
- Use mock data for UI development
- Mock Docker operations until Phase 4
- Mock Git operations until Phase 3

### Docker Integration Notes

#### Container Requirements
- Based on existing claude-docker image
- Mount workspace as volume
- Copy environment files (.env, .env.local)
- Proper signal handling for clean shutdown

#### Security Considerations
- Validate all file paths
- Sandbox container access
- No privileged container mode
- Secure environment variable handling

### Performance Guidelines

#### UI Responsiveness
- Async operations for I/O
- Non-blocking UI updates
- Efficient terminal rendering
- Smooth scrolling and navigation

#### Memory Management
- Limit log buffer sizes
- Clean up unused sessions
- Efficient data structures
- Monitor container resource usage



## Development Guidelines

### TTY Conflict Prevention Rule 🚨

**CRITICAL**: When implementing TUI applications that need to spawn interactive processes, always check for TTY conflicts during planning phase.

#### Problem Description
Running interactive processes (especially Docker with `-it` flag) from within a TUI application causes terminal control conflicts:
- **Symptoms**: Garbled ANSI escape sequences in TUI input fields
- **Root Cause**: Multiple processes competing for TTY control (Ratatui vs subprocess)
- **Common Triggers**: `docker run -it`, `ssh -t`, interactive shells, CLI tools expecting TTY

#### Implementation Strategy

**When Planning Interactive Features:**
1. **Identify TTY Dependencies**: List all subprocess calls that might need TTY
2. **Plan Isolation Strategy**: Choose appropriate isolation method
3. **Design Fallback Options**: Always provide non-interactive alternatives

**TTY Isolation Techniques:**

1. **New Terminal Window** (Recommended for user-facing interactions)
   ```rust
   // macOS
   std::process::Command::new("osascript")
       .args(["-e", "tell application \"Terminal\" to do script \"docker run -it ...\""])
       .status()?;
   
   // Linux - try common terminals
   for terminal in ["gnome-terminal", "xterm", "konsole"] {
       if let Ok(_) = std::process::Command::new(terminal)
           .args(["--", "bash", "-c", "docker run -it ..."])
           .status() { break; }
   }
   ```

2. **TTY Detachment** (For background processes)
   ```rust
   // Use setsid to create new session
   std::process::Command::new("setsid")
       .args(["docker", "run", "--rm", "image"])
       .status()?;
   
   // Or spawn without TTY allocation
   std::process::Command::new("docker")
       .args(["run", "--rm", "image"])  // No -it flags
       .status()?;
   ```

3. **Alternative Interfaces** (API-based approaches)
   ```rust
   // Use Docker API instead of CLI
   use bollard::Docker;
   let docker = Docker::connect_with_local_defaults()?;
   
   // Use REST APIs instead of interactive CLIs
   // Use file-based authentication instead of interactive login
   ```

#### Real-World Example

**Authentication Setup Implementation:**
- **Problem**: OAuth login via `docker run -it` caused TUI garbled input
- **Solution**: Platform-specific terminal spawning with automatic status monitoring
- **Files**: `src/app/state.rs:1077-1125`, `src/components/auth_setup.rs`
- **Result**: Clean TUI + separate terminal for OAuth flow

#### Checklist for Interactive Features ✅

Before implementing any feature that spawns processes:

- [ ] **TTY Analysis**: Does this process expect interactive terminal?
- [ ] **User Experience**: Should this be visible to user or background?
- [ ] **Platform Support**: How will this work on macOS/Linux/Windows?
- [ ] **Fallback Strategy**: What if terminal spawning fails?
- [ ] **Status Monitoring**: How will TUI know when process completes?
- [ ] **Error Handling**: How to handle process failures gracefully?
- [ ] **Testing Strategy**: How to test without actual TTY conflicts?

#### Common Patterns to Avoid ❌

```rust
// DON'T: This will cause TTY conflicts in TUI
std::process::Command::new("docker")
    .args(["run", "-it", "image"])
    .status()?;

// DON'T: Interactive shells from TUI
std::process::Command::new("bash")
    .arg("-i")  // Interactive flag
    .status()?;

// DON'T: SSH with TTY allocation
std::process::Command::new("ssh")
    .args(["-t", "user@host"])
    .status()?;
```

#### Safe Patterns to Use ✅

```rust
// DO: Spawn in new terminal for user interactions
spawn_in_new_terminal("docker run -it image");

// DO: Use non-interactive modes when possible
std::process::Command::new("docker")
    .args(["run", "--rm", "image", "non-interactive-command"])
    .output()?;

// DO: Use APIs instead of CLI when available
use bollard::Docker;
let docker = Docker::connect_with_local_defaults()?;
```

This rule prevents a class of bugs that are difficult to debug and significantly impact user experience in TUI applications.

### Troubleshooting Guide

#### Common Issues
1. **Compilation Errors**: Run `just check` before commits
2. **UI Artifacts**: Ensure proper terminal cleanup on exit
3. **Container Issues**: Validate Docker daemon connectivity
4. **Git Issues**: Check repository permissions and worktree conflicts

#### Debug Mode
- Set `RUST_LOG=debug` for detailed logging
- Use `just test-verbose` for test debugging
- Monitor container logs in real-time

### Future Enhancements

#### Post-MVP Features
- Session templates with pre-configured environments
- MCP server configuration management
- Resource usage monitoring
- Web UI companion app
- GitHub Codespaces integration

### Contribution Guidelines

#### Before Starting Work
1. Check the current phase in the PRD
2. Run `just check` to ensure clean state
3. Create feature branch: `git checkout -b feature/description`
4. Write tests first (TDD approach)

#### Before Submitting
1. All tests must pass: `just test`
2. Code must be formatted: `just fmt`
3. No lint warnings: `just lint`
4. Update documentation as needed

#### Code Review Checklist
- [ ] Tests cover new functionality
- [ ] Error handling is comprehensive
- [ ] Documentation is complete
- [ ] Performance impact considered
- [ ] Security implications reviewed

## Health Check Information
This project is in active development. Phase 1 (UI Foundation) is complete with full test coverage. Ready to proceed with Phase 3 (Git Integration) as the next priority.