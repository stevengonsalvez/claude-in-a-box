# Tmux Integration

## Overview

Claude-in-a-box integrates tmux sessions for interactive development sessions, providing a split-pane interface with live preview. Each Claude session runs in an isolated tmux session alongside a Docker container, giving you:

- **Live preview** of terminal output in the TUI
- **Seamless attach/detach** with Ctrl+Q
- **Scroll mode** for reviewing session history
- **Status indicators** showing tmux session state

## Architecture

### Components

- **TmuxSession**: Manages individual tmux sessions (lifecycle, PTY, content capture)
- **TmuxPreviewPane**: TUI component displaying live tmux output
- **AttachHandler**: Manages TUI suspend/resume for full terminal takeover
- **Git Worktrees**: Each session runs in an isolated git worktree

### Layout

```
┌─────────────────────────────────────────────┐
│              Status Bar                     │
├──────────────┬──────────────────────────────┤
│              │                              │
│  Session     │   Tmux Preview               │
│  List        │   (Live Output)              │
│  (40%)       │   (60%)                      │
│              │                              │
│  ● Session 1 │   $ claude                   │
│  ○ Session 2 │   > Working on...            │
│  🔗 Session 3│   > Making changes...        │
│              │                              │
├──────────────┴──────────────────────────────┤
│           Bottom Logs Area                  │
├─────────────────────────────────────────────┤
│           Menu Bar                          │
└─────────────────────────────────────────────┘
```

## Workflow

### Creating a Session

1. Press `n` to create a new session
2. Select repository and branch
3. Session creates:
   - Git worktree in isolated directory
   - Docker container with development environment
   - Tmux session running in the worktree
4. Preview pane shows live tmux output

### Viewing Sessions

- **Status Indicators**:
  - `🔗` - Currently attached to tmux
  - `●` - Tmux session running
  - `○` - No tmux session (container only)

- **Preview Pane**: Shows last N lines of tmux output
- **Auto-scroll**: Enabled in normal mode
- **Updates**: Every 100ms (configurable)

### Attaching to a Session

1. Select a session in the list
2. Press `a` to attach
3. TUI suspends, tmux session takes full terminal
4. Work normally in Claude/shell
5. Press `Ctrl+Q` to detach
6. TUI resumes, back to session list

### Scroll Mode

Review session history without attaching:

1. Press `Shift+Up` or `Shift+Down` to enter scroll mode
2. Use arrow keys or Page Up/Down to navigate
3. Mouse wheel scrolling supported
4. Press `ESC` to exit scroll mode

## Keyboard Shortcuts

### Main View

| Key | Action |
|-----|--------|
| `n` | Create new session |
| `a` | Attach to selected tmux session |
| `Tab` | Switch focus between panes |
| `Shift+↑/↓` | Scroll tmux preview |
| `?` | Show help |

### In Tmux Session (Attached)

| Key | Action |
|-----|--------|
| `Ctrl+Q` | Detach and return to TUI |
| (All other keys passed to tmux) | |

### Scroll Mode

| Key | Action |
|-----|--------|
| `↑/↓` or `k/j` | Scroll one line |
| `Page Up/Down` | Scroll one page |
| `Home/End` | Go to top/bottom |
| `ESC` | Exit scroll mode |

## Configuration

Edit `.claude-in-a-box/config.toml`:

```toml
[tmux]
# Detach key combination (default: "ctrl-q")
detach_key = "ctrl-q"

# Preview update interval in milliseconds (default: 100)
preview_update_interval_ms = 100

# Tmux history limit in lines (default: 10000)
history_limit = 10000

# Enable mouse scrolling (default: true)
enable_mouse_scroll = true
```

### Configuration Options

**detach_key**
- Key combination to detach from tmux
- Default: `"ctrl-q"`
- Supports: `"ctrl-*"` combinations

**preview_update_interval_ms**
- How often to refresh preview pane (milliseconds)
- Default: `100`
- Lower = more responsive, higher CPU usage
- Recommended range: 50-500ms

**history_limit**
- Number of lines kept in tmux scrollback
- Default: `10000`
- Increase for longer sessions, decrease to save memory

**enable_mouse_scroll**
- Enable mouse wheel scrolling in scroll mode
- Default: `true`

## Session Isolation

Each tmux session runs in an isolated environment:

- **Git Worktree**: Separate working directory per session
- **Tmux Session**: Named `tmux_<branch_name>`
- **Docker Container**: Isolated filesystem and processes
- **Branch**: Dedicated git branch (`claude/<session_name>`)

Changes made in one session don't affect others.

## Troubleshooting

### Tmux not found

**Problem**: Error message "tmux not installed"

**Solution**: Install tmux:
```bash
# macOS
brew install tmux

# Ubuntu/Debian
sudo apt install tmux

# Fedora/RHEL
sudo dnf install tmux
```

### Orphaned tmux sessions

**Problem**: Tmux sessions remain after deleting sessions

**Solution**: List and clean up manually:
```bash
# List all tmux sessions
tmux ls

# Kill specific session
tmux kill-session -t tmux_<branch_name>

# Kill all sessions starting with tmux_
tmux ls | grep '^tmux_' | cut -d: -f1 | xargs -I {} tmux kill-session -t {}
```

Or use the cleanup command in claude-in-a-box (press `x`).

### Garbled terminal output

**Problem**: Terminal shows strange characters after detaching

**Solution**:
1. Ensure `TERM` environment variable is set correctly
2. Try resetting the terminal: `reset` or `Ctrl+L`
3. Check tmux configuration doesn't override terminal settings

### Preview not updating

**Problem**: Tmux preview pane shows stale content

**Solution**:
1. Check tmux session is actually running: `tmux ls`
2. Verify update interval isn't too high in config
3. Try switching to another session and back
4. Restart claude-in-a-box

### Detach key not working

**Problem**: Ctrl+Q doesn't detach from tmux

**Solution**:
1. Ensure tmux session was started by claude-in-a-box
2. Check config file for correct `detach_key` setting
3. Try alternative: `Ctrl+B` then `D` (default tmux detach)
4. Check terminal emulator doesn't intercept Ctrl+Q

## Technical Details

### Session Lifecycle

1. **Creation**:
   - Git worktree created in `~/.claude-in-a-box/worktrees/by-name/<repo>--<branch>--<uuid>`
   - Tmux session starts in worktree directory
   - Session name: `tmux_<sanitized_branch_name>`
   - Stores in `AppState.tmux_sessions` HashMap

2. **Running**:
   - Preview updates every 100ms via `tmux capture-pane -p`
   - Content stored in `Session.preview_content`
   - Status tracked in `Session.is_attached`

3. **Cleanup**:
   - Tmux session killed: `tmux kill-session -t <name>`
   - Worktree removed via WorktreeManager
   - Entry removed from tmux_sessions map

### PTY Management

- Uses `portable-pty` crate for cross-platform PTY support
- Detach detection: scans stdin for byte `0x11` (Ctrl+Q)
- Terminal state restoration handled by AttachHandler

### Content Capture

Normal mode:
```bash
tmux capture-pane -p -e -J -t <session_name>
```

Scroll mode (full history):
```bash
tmux capture-pane -p -e -J -S - -t <session_name>
```

Flags:
- `-p`: Print to stdout
- `-e`: Include escape sequences (colors)
- `-J`: Join wrapped lines
- `-S -`: Start from beginning of history

## Best Practices

1. **Use descriptive branch names**: They become tmux session names
2. **Regular cleanup**: Delete old sessions to avoid clutter
3. **Check status indicators**: `●` shows tmux is ready
4. **Use scroll mode**: Review history without attaching
5. **Ctrl+Q habit**: Always detach cleanly, don't close terminal

## Limitations

- **No nested tmux**: Don't run tmux inside tmux sessions
- **Terminal size**: Preview pane size limited by TUI layout
- **Performance**: Very active sessions may lag preview updates
- **Platform**: Requires Unix-like system (Linux, macOS)

## Advanced Usage

### Custom Tmux Configuration

Sessions respect your `~/.tmux.conf` settings:

```bash
# ~/.tmux.conf
set -g mouse on
set -g history-limit 50000
set -g status-style bg=black,fg=white
```

### Session Recovery

If TUI crashes but tmux session survives:

```bash
# List sessions
tmux ls

# Attach manually
tmux attach -t tmux_<branch_name>

# Resume in claude-in-a-box
# Session will be detected and integrated
```

## Further Reading

- [Tmux Cheat Sheet](https://tmuxcheatsheet.com/)
- [Git Worktrees](https://git-scm.com/docs/git-worktree)
- [PTY Basics](https://en.wikipedia.org/wiki/Pseudoterminal)

## Support

For issues or questions:
- Check this documentation first
- Review GitHub issues
- File a new issue with reproduction steps
