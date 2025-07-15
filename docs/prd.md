# Claude-in-a-Box MVP - Product Requirements Document

## Executive Summary
Claude-in-a-Box is a terminal-based development environment manager that creates isolated Docker containers running Claude Code with dedicated git worktrees. It enables developers to quickly spin up, manage, and switch between multiple AI-assisted coding sessions across different projects.

## Problem Statement
- Developers need isolated environments for Claude Code sessions to avoid workspace conflicts
- Managing multiple concurrent AI coding sessions across projects is cumbersome
- No unified interface to track and access dockerized development environments
- Manual worktree and container management is error-prone and time-consuming

## MVP Features

### Core Functionality
1. **Session Management**
   - Create new dockerized Claude Code sessions with automatic git worktree creation
   - List all sessions across multiple workspaces with status indicators
   - Start/stop/delete sessions
   - Persist session state between application restarts

2. **Container Operations**
   - Build and use custom Claude Code Docker image (based on existing claude-docker)
   - Mount git worktrees as workspace volumes
   - Copy environment files (.env, .env.local) into containers
   - Stream container logs in real-time

3. **Interactive Terminal**
   - Attach to running container's TTY for direct interaction
   - Detach without stopping the container
   - Pass input to Claude Code process
   - View process output and errors

4. **Workspace Organization**
   - Support multiple git repositories/workspaces
   - Group sessions by workspace in UI
   - Show git diff statistics per session
   - Automatic branch naming convention (claude/task-name)

### User Interface
- **Split-pane TUI layout**
  - Left: Hierarchical session list grouped by workspace
  - Right: Context-sensitive view (logs/terminal)
- **Keyboard navigation**
  - Vim-style keys for navigation
  - Quick actions (n: new, a: attach, s: stop, d: delete)
  - Tab switching between views
- **Visual indicators**
  - Session status (running/stopped/error)
  - Active git changes count
  - Container resource usage (future)

## Implementation Order

### Phase 1: UI Foundation (Week 1)
1. **Basic TUI Skeleton**
   - Ratatui app setup with event loop
   - Terminal initialization/restoration
   - Basic layout with split panes
   - Mock data structures

2. **Session List Component**
   - Scrollable list widget
   - Workspace grouping/headers
   - Selection highlighting
   - Status indicators (● running, ⏸ stopped, ✗ error)

3. **Navigation & Controls**
   - Keyboard event handling
   - Vim-style navigation (j/k, g/G)
   - Bottom menu bar
   - Help overlay (?)

4. **Right Pane Views**
   - Empty state ("Select a session")
   - Log viewer with scrolling
   - Tab system for future views

### Phase 2: Data Models & State (Week 1-2)
5. **Session Model**
   - Session struct with all properties
   - Workspace grouping logic
   - Status enum and transitions

6. **Mock Data Layer**
   - Fake session generator
   - State management in app
   - Session CRUD operations (in-memory)

7. **UI-State Binding**
   - Update UI from state changes
   - Handle user actions
   - Loading states

### Phase 3: Git Integration (Week 2)
8. **Workspace Detection**
   - Scan filesystem for git repos
   - Validate git repositories
   - Store workspace registry

9. **Worktree Operations**
   - Create worktree with branch
   - List existing worktrees
   - Delete/cleanup worktrees

10. **Git Status Integration**
    - Get diff stats
    - Branch information
    - Display in UI

### Phase 4: Docker Integration (Week 3)
11. **Docker Client Setup**
    - Bollard initialization
    - Connection validation
    - Error handling

12. **Container Lifecycle**
    - Create container from image
    - Start/stop operations
    - Delete containers
    - Status monitoring

13. **Log Streaming**
    - Attach to container logs
    - Buffer management
    - Display in UI

### Phase 5: Interactive Features (Week 3-4)
14. **TTY Attachment**
    - Raw terminal mode
    - I/O redirection
    - Detach mechanism
    - Signal handling

15. **Environment Management**
    - Copy .env files
    - Mount configuration
    - Volume management

### Phase 6: Persistence & Polish (Week 4)
16. **State Persistence**
    - Save/load sessions
    - Configuration file
    - Crash recovery

17. **Error Handling**
    - User-friendly messages
    - Recovery suggestions
    - Validation

18. **Performance & UX**
    - Async operations
    - Progress indicators
    - Smooth scrolling

## Technical Stack

### Core Technologies
- **Language**: Rust (performance, memory safety, single binary)
- **TUI Framework**: Ratatui 0.26+ (modern, actively maintained)
- **Terminal Handler**: Crossterm 0.27+ (cross-platform)
- **Async Runtime**: Tokio 1.36+ (concurrent container management)

### Key Dependencies
```toml
[dependencies]
# Core
tokio = { version = "1.36", features = ["full"] }
ratatui = "0.26"
crossterm = "0.27"

# Docker
bollard = "0.16"

# Git operations  
git2 = "0.18"

# Data persistence
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"

# Utils
anyhow = "1.0"
thiserror = "1.0"
directories = "5.0"
uuid = { version = "1.5", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }

```


## Data Storage

Session State: JSON file in ~/.claude-box/sessions.json
Configuration: TOML in ~/.claude-box/config.toml
Worktrees: ~/.claude-box/worktrees/{session-id}/

## User Flows
### Primary Flow: Create and Use Session

Launch claude-box from terminal
Select workspace or add new one
Press 'n' to create new session
Enter session name
System creates worktree, starts container
Press 'a' to attach to container
Interact with Claude Code
Press 'Ctrl+D' to detach (container keeps running)

###  Secondary Flows

Resume Session: Select stopped session → Press 's' to start
View Logs: Select running session → Logs auto-display
Quick Switch: Number keys (1-9) for quick session selection

## UI Mockup
┌─ Claude-in-a-Box ──────────────────────────────────────────┐
│ Workspaces         │ Session: project1/fix-auth            │
│                    │ Status: ● Running                      │
│ ▼ project1         │ Branch: claude/fix-auth                │
│   ● fix-auth       │ Changes: +42 -13                       │
│   ⏸ add-feature    ├────────────────────────────────────────┤
│   ✗ debug-issue    │ Logs                                   │
│                    │ ─────────────────────────────────────  │
│ ▶ project2         │ Starting Claude Code environment...    │
│   ● refactor-api   │ Loading MCP servers...                 │
│                    │ Ready! Attached to container.          │
│ ▶ project3         │                                        │
│   ⏸ new-feature    │ > claude help                          │
│   ⏸ bug-fix        │ Available commands:                    │
│                    │   help     Show this help message      │
├────────────────────┴────────────────────────────────────────┤
│ [n]ew [a]ttach [s]tart/stop [d]elete [w]orkspace [q]uit    │
└─────────────────────────────────────────────────────────────┘

## Success Metrics

Time to create new session < 10 seconds
Zero data loss on application crash
Support 20+ concurrent sessions
Sub-second workspace switching

## Future Enhancements (Post-MVP)

Session templates with pre-configured environments
MCP server configuration management
Multi-user support with session sharing
GitHub Codespaces integration
Session recording and playback
Resource usage monitoring
Custom Docker image builder UI
Web UI companion app

## Risks and Mitigations

Risk: Docker daemon dependency

Mitigation: Clear error messages and setup guide


Risk: Worktree conflicts

Mitigation: Unique branch naming, conflict detection


Risk: Terminal compatibility

Mitigation: Fallback rendering modes


Risk: Container resource exhaustion

Mitigation: Resource limits, monitoring alerts



## howDevelopment Milestones
Milestone 1: UI Complete (Week 1)

 Basic TUI with navigation
 Session list with mock data
 Keyboard shortcuts working
 Help overlay

Milestone 2: Git Ready (Week 2)

 Workspace detection
 Worktree creation/deletion
 Git status integration

Milestone 3: Docker Integration (Week 3)

 Container lifecycle management
 Log streaming
 Environment mounting

Milestone 4: MVP Complete (Week 4)

 TTY attachment working
 State persistence
 Error handling
 Documentation