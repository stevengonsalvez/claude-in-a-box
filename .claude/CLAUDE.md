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