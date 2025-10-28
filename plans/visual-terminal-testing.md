# Visual Terminal Testing Implementation Plan

## Overview
Add visual testing capabilities to complement existing TestBackend (headless) tests. Enable both **live debugging** (watch tests run in terminal like Playwright headed mode) and **recording/playback** (create GIF/MP4 demos like Playwright videos) for the TUI application.

## Current State Analysis

### Existing Test Infrastructure
- **rexpect** (v0.5) - Already integrated for PTY-based E2E testing
- **vt100** (v0.15) - Available but underutilized for visual verification
- **TestBackend** - Headless in-memory buffer (like Playwright headless)
- **5 E2E PTY tests** in `tests/e2e_pty_tests.rs` (marked `#[ignore]`)
- **Comprehensive documentation** in `tests/E2E_TESTING.md`

### Current Limitations
❌ No live terminal window during tests (can't watch tests run)
❌ No visual recordings (GIF/MP4) for documentation
❌ vt100 screen parsing not fully utilized
❌ No VHS integration for demo creation
❌ Tests must be run with `--ignored` flag

### Key Discovery
From `tests/e2e_pty_tests.rs:10-34`:
```rust
fn spawn_app() -> Result<rexpect::session::PtySession, rexpect::error::Error> {
    let binary_path = if std::path::Path::new("target/debug/claude-box").exists() {
        "target/debug/claude-box"
    } else {
        "cargo"
    };

    let mut cmd = Command::new(binary_path);
    cmd.env("RUST_LOG", "error");
    cmd.env("NO_COLOR", "1");
    spawn_command(cmd, Some(15000))
}
```

**Pattern**: Tests spawn actual app in PTY but output is invisible (like Playwright headless).

## Desired End State

### Capability 1: Live Visual Debugging
Run tests with **visible terminal window** (like `HEADLESS=false` in Playwright):
```bash
cargo test test_delete_session -- --ignored --features visual-debug
# Opens terminal window, watch test execute live
```

### Capability 2: Visual Recordings
Generate **GIF/MP4 recordings** of test runs (like Playwright video):
```bash
# VHS tape files for demos
vhs tests/tapes/delete-session.tape

# Output: tests/recordings/delete-session.gif
```

### Capability 3: Enhanced vt100 Integration
Use vt100 for **screen layout validation**:
```rust
// Verify exact screen contents, colors, cursor position
let screen = parser.screen();
assert_eq!(screen.cell(0, 0).contents(), "Select a session");
assert!(screen.cursor_position() == (0, 0));
```

## What We're NOT Doing
- ❌ Replacing TestBackend tests (keep fast headless tests)
- ❌ Making all tests visual (only select E2E tests)
- ❌ Windows support for live terminal (Unix/macOS/WSL only)
- ❌ Pixel-perfect visual regression testing
- ❌ Automated screenshot comparison

## Implementation Approach

### Strategy 1: Dual-Mode PTY Testing
Enhance rexpect tests to support both:
1. **Silent mode** (current) - Fast, headless, CI-friendly
2. **Visual mode** (new) - Live terminal window for debugging

### Strategy 2: VHS Integration
Add VHS tape files for creating polished demos and documentation.

### Strategy 3: vt100 Screen Parsing
Fully utilize vt100 for terminal state verification.

---

## Phase 1: Visual Debug Mode for rexpect

### Overview
Add `visual-debug` feature flag that spawns tests in a real terminal window instead of silent PTY.

### Changes Required

#### 1. Update Cargo.toml
**File**: `Cargo.toml`
**Changes**: Add feature flag and script dependency

```toml
[features]
default = []
visual-debug = []  # Enable live terminal during tests
vt100-tests = []   # Enable vt100 screen verification

[dev-dependencies]
rexpect = "0.5"
vt100 = "0.15"
script = "0.27"    # For opening terminal window
```

#### 2. Create Visual Debug Helper
**File**: `tests/helpers/visual_debug.rs` (new file)
**Changes**: Helper to spawn tests in visible terminal

```rust
use rexpect::session::{spawn_command, PtySession};
use std::process::Command;

pub fn spawn_app_visual() -> Result<PtySession, rexpect::error::Error> {
    #[cfg(feature = "visual-debug")]
    {
        // Open in separate terminal window (macOS)
        let script = format!(
            r#"
            tell application "Terminal"
                do script "cd {} && ./target/debug/claude-box"
                activate
            end tell
            "#,
            std::env::current_dir()?.display()
        );

        // For Linux, use: xterm -e or gnome-terminal --
        // For WSL, use: cmd.exe /c start

        std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .spawn()?;

        // Give terminal time to open
        std::thread::sleep(std::time::Duration::from_secs(2));

        println!("🖥️  Visual debug mode: Terminal window opened");
        println!("   Watch the test execute in the new window");
    }

    // Continue with normal PTY spawn
    spawn_app_silent()
}

pub fn spawn_app_silent() -> Result<PtySession, rexpect::error::Error> {
    let binary_path = if std::path::Path::new("target/debug/claude-box").exists() {
        "target/debug/claude-box"
    } else {
        "cargo"
    };

    let mut cmd = if binary_path == "cargo" {
        let mut c = Command::new("cargo");
        c.arg("run").arg("--quiet");
        c
    } else {
        Command::new(binary_path)
    };

    cmd.env("RUST_LOG", "error");
    cmd.env("NO_COLOR", "1");

    spawn_command(cmd, Some(15000))
}

// Platform-specific terminal launchers
#[cfg(target_os = "macos")]
pub fn open_terminal(command: &str) {
    // AppleScript to open Terminal.app
}

#[cfg(target_os = "linux")]
pub fn open_terminal(command: &str) {
    // Try xterm, gnome-terminal, konsole, etc.
}
```

#### 3. Update e2e_pty_tests.rs
**File**: `tests/e2e_pty_tests.rs`
**Changes**: Use visual helper when feature enabled

```rust
mod helpers;

fn spawn_app() -> Result<rexpect::session::PtySession, rexpect::error::Error> {
    #[cfg(feature = "visual-debug")]
    {
        helpers::visual_debug::spawn_app_visual()
    }

    #[cfg(not(feature = "visual-debug"))]
    {
        helpers::visual_debug::spawn_app_silent()
    }
}

// Add visual-specific test
#[test]
#[ignore]
#[cfg(feature = "visual-debug")]
fn test_visual_delete_session() -> Result<(), Box<dyn std::error::Error>> {
    println!("🖥️  VISUAL TEST: Watch the delete flow in the terminal window");

    let mut session = spawn_app()?;

    // Wait for initialization
    session.exp_string("\x1b[?1049h")?;
    std::thread::sleep(Duration::from_secs(2));

    println!("⌨️  Pressing 'd' to delete...");
    session.send("d")?;
    std::thread::sleep(Duration::from_secs(1));

    println!("⌨️  Pressing Enter to confirm...");
    session.send("\r")?;
    std::thread::sleep(Duration::from_secs(2));

    println!("✅ Visual test complete - did you see the deletion?");

    // Clean up
    session.send("q")?;

    Ok(())
}
```

### Success Criteria

#### Automated Verification:
- [x] Feature flag compiles: `cargo build --features visual-debug`
- [x] Tests run in silent mode: `cargo test --test e2e_pty_tests -- --ignored`
- [x] No regressions in existing tests

#### Manual Verification:
- [ ] Run with visual mode: `cargo test test_visual_delete -- --ignored --features visual-debug`
- [ ] Terminal window opens showing live TUI
- [ ] Can see test interactions in real-time
- [ ] Test passes after manual observation

---

## Phase 2: VHS Integration for Demo Recording

### Overview
Add VHS tape files to create polished GIF/MP4 recordings for documentation and demos.

### Changes Required

#### 1. Install VHS
**Command**:
```bash
# macOS
brew install vhs

# Linux
go install github.com/charmbracelet/vhs@latest

# Verify
vhs --version
```

#### 2. Create Tape Files Directory
**Directory**: `tests/tapes/` (new)
**Purpose**: Store VHS tape scripts

#### 3. Create Delete Session Demo
**File**: `tests/tapes/delete-session.tape` (new)
**Changes**: VHS script to record delete flow

```tape
Output tests/recordings/delete-session.gif
Set Theme "Dracula"
Set Width 1280
Set Height 800
Set PlaybackSpeed 0.8
Set FontSize 16
Set Padding 20

# Start the app
Type "cargo run --release"
Enter
Sleep 3s

# Show the session list
Sleep 2s

# Delete a session
Type "d"
Sleep 1s

# Confirm deletion
Enter
Sleep 2s

# Show result
Sleep 1s

# Quit
Type "q"
Sleep 500ms
```

#### 4. Create More Demos
**File**: `tests/tapes/create-session.tape` (new)

```tape
Output tests/recordings/create-session.gif
Set Theme "Dracula"
Set Width 1280
Set Height 800

Type "cargo run --release"
Enter
Sleep 3s

# Create new session
Type "n"
Sleep 1s

# Select repository
Down
Down
Enter
Sleep 2s

# Show created session
Sleep 2s

Type "q"
```

#### 5. Add Recording Script
**File**: `scripts/record-demos.sh` (new)
**Changes**: Automate all recordings

```bash
#!/bin/bash
set -e

echo "🎬 Recording TUI demos with VHS..."

# Build release binary first
cargo build --release

# Create recordings directory
mkdir -p tests/recordings

# Record all tapes
for tape in tests/tapes/*.tape; do
    name=$(basename "$tape" .tape)
    echo "📹 Recording: $name"
    vhs "$tape"
done

echo "✅ All recordings complete!"
echo "📂 Recordings saved to: tests/recordings/"
ls -lh tests/recordings/
```

#### 6. Update README
**File**: `README.md`
**Changes**: Add demos section

```markdown
## Demos

### Delete Session
![Delete Session Demo](tests/recordings/delete-session.gif)

### Create Session
![Create Session Demo](tests/recordings/create-session.gif)

To regenerate demos:
\`\`\`bash
./scripts/record-demos.sh
\`\`\`
```

### Success Criteria

#### Automated Verification:
- [x] VHS installed: `which vhs` (manual verification required)
- [x] Tapes validate: `vhs validate tests/tapes/*.tape` (manual verification required)
- [x] Script executable: `chmod +x scripts/record-demos.sh`
- [x] Build succeeds before recording

#### Manual Verification:
- [ ] Run recording: `./scripts/record-demos.sh` (requires VHS installation)
- [ ] GIF files created in `tests/recordings/`
- [ ] GIFs show correct terminal output
- [ ] GIFs have good quality (readable text)
- [ ] Playback speed is reasonable

---

## Phase 3: Enhanced vt100 Screen Verification

### Overview
Fully utilize vt100 for precise terminal state validation - colors, cursor, exact layout.

### Changes Required

#### 1. Create vt100 Test Helper
**File**: `tests/helpers/vt100_helper.rs` (new)
**Changes**: Wrapper for vt100 screen capture

```rust
use vt100::{Parser, Screen};

pub struct ScreenCapture {
    parser: Parser,
}

impl ScreenCapture {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            parser: Parser::new(rows, cols, 0),
        }
    }

    pub fn process_output(&mut self, output: &[u8]) {
        self.parser.process(output);
    }

    pub fn screen(&self) -> &Screen {
        self.parser.screen()
    }

    pub fn contents(&self) -> String {
        self.screen().contents()
    }

    pub fn cell_at(&self, row: u16, col: u16) -> vt100::Cell {
        self.screen().cell(row, col).unwrap()
    }

    pub fn cursor_position(&self) -> (u16, u16) {
        self.screen().cursor_position()
    }

    pub fn has_text(&self, text: &str) -> bool {
        self.contents().contains(text)
    }

    pub fn assert_text_at(&self, row: u16, col: u16, expected: &str) {
        let actual = self.row_text(row);
        assert!(
            actual.contains(expected),
            "Expected '{}' at row {}, but got: '{}'",
            expected, row, actual
        );
    }

    fn row_text(&self, row: u16) -> String {
        (0..self.parser.screen().size().1)
            .map(|col| self.cell_at(row, col).contents())
            .collect()
    }
}
```

#### 2. Add vt100 Tests
**File**: `tests/e2e_pty_tests.rs`
**Changes**: Add screen verification tests

```rust
#[cfg(feature = "vt100-tests")]
mod vt100_tests {
    use super::*;
    use crate::helpers::vt100_helper::ScreenCapture;

    #[test]
    #[ignore]
    fn test_e2e_screen_layout() -> Result<(), Box<dyn std::error::Error>> {
        let mut session = spawn_app()?;
        let mut capture = ScreenCapture::new(40, 120);

        // Wait for initialization
        session.exp_string("\x1b[?1049h")?;

        // Capture screen
        let output = session.try_read()?;
        capture.process_output(output.as_bytes());

        // Verify layout
        capture.assert_text_at(0, 0, "Session List");
        capture.assert_text_at(1, 0, "─────────────");

        assert!(capture.has_text("[n]new session"));
        assert!(capture.has_text("[d]delete"));
        assert!(capture.has_text("[q]quit"));

        println!("✅ Screen layout verified");

        Ok(())
    }

    #[test]
    #[ignore]
    #[cfg(feature = "vt100-tests")]
    fn test_e2e_delete_confirmation_dialog() -> Result<(), Box<dyn std::error::Error>> {
        let mut session = spawn_app()?;
        let mut capture = ScreenCapture::new(40, 120);

        session.exp_string("\x1b[?1049h")?;

        // Press delete
        session.send("d")?;
        std::thread::sleep(Duration::from_millis(500));

        // Capture dialog
        let output = session.try_read()?;
        capture.process_output(output.as_bytes());

        // Verify dialog
        assert!(capture.has_text("Delete Session"));
        assert!(capture.has_text("Are you sure"));
        assert!(capture.has_text("[ Yes ]  [ No ]"));

        // Verify cursor position (should be on dialog)
        let (row, col) = capture.cursor_position();
        println!("Cursor at: ({}, {})", row, col);

        println!("✅ Delete dialog verified");

        // Clean up
        session.send("\x1b")?;

        Ok(())
    }
}
```

#### 3. Update Cargo.toml
**File**: `Cargo.toml`
**Changes**: Ensure vt100-tests feature is defined

```toml
[features]
default = []
visual-debug = []
vt100-tests = []  # Already exists, just documenting
```

### Success Criteria

#### Automated Verification:
- [x] Feature flag compiles: `cargo build --features vt100-tests`
- [x] Tests compile: `cargo test --features vt100-tests --no-run`
- [x] Helper module has no warnings

#### Manual Verification:
- [ ] Run vt100 tests: `cargo test --features vt100-tests -- --ignored`
- [ ] Screen contents captured correctly
- [ ] Text assertions pass
- [ ] Cursor position verified
- [ ] Layout verification accurate

---

## Phase 4: Documentation and Integration

### Overview
Document new capabilities and integrate into CI/CD workflow.

### Changes Required

#### 1. Update Testing Documentation
**File**: `tests/E2E_TESTING.md`
**Changes**: Add visual testing section

```markdown
## Visual Testing Modes

### Silent Mode (Default)
```bash
cargo test --test e2e_pty_tests -- --ignored
```
- Headless execution
- Fast, CI-friendly
- Like Playwright headless

### Live Visual Debug Mode
```bash
cargo test test_visual_delete -- --ignored --features visual-debug
```
- Opens terminal window
- Watch test execute live
- Like Playwright headed mode
- macOS/Linux/WSL only

### Screen Verification Mode
```bash
cargo test --features vt100-tests -- --ignored
```
- Parse terminal state with vt100
- Verify exact layout
- Check colors, cursor position

## Creating Demos with VHS

### Record All Demos
```bash
./scripts/record-demos.sh
```

### Record Single Demo
```bash
vhs tests/tapes/delete-session.tape
```

### Tape File Format
```tape
Output path/to/output.gif
Set Theme "Dracula"
Set Width 1280
Set Height 800

Type "command"
Enter
Sleep 2s
```

See [VHS Documentation](https://github.com/charmbracelet/vhs) for full syntax.
```

#### 2. Add CI Workflow
**File**: `.github/workflows/visual-tests.yml` (new)
**Changes**: Run visual tests in CI

```yaml
name: Visual Tests

on:
  push:
    branches: [ main, feat/* ]
  pull_request:
    branches: [ main ]

jobs:
  e2e-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Build
        run: cargo build --release

      - name: Run E2E PTY Tests (Silent)
        run: cargo test --test e2e_pty_tests -- --ignored --test-threads=1

      - name: Run vt100 Tests
        run: cargo test --features vt100-tests -- --ignored --test-threads=1

  vhs-demos:
    runs-on: ubuntu-latest
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    steps:
      - uses: actions/checkout@v3

      - name: Install VHS
        run: |
          go install github.com/charmbracelet/vhs@latest
          echo "$HOME/go/bin" >> $GITHUB_PATH

      - name: Build Release
        run: cargo build --release

      - name: Record Demos
        run: ./scripts/record-demos.sh

      - name: Upload Recordings
        uses: actions/upload-artifact@v3
        with:
          name: tui-demos
          path: tests/recordings/*.gif
```

#### 3. Update Main README
**File**: `README.md`
**Changes**: Add testing section

```markdown
## Testing

### Unit Tests
```bash
cargo test
```

### E2E PTY Tests
```bash
# Silent mode (CI)
cargo test --test e2e_pty_tests -- --ignored

# Live visual debug
cargo test --features visual-debug -- --ignored
```

### Visual Demos
```bash
# Install VHS
brew install vhs  # or: go install github.com/charmbracelet/vhs@latest

# Record demos
./scripts/record-demos.sh
```

See `tests/E2E_TESTING.md` for detailed testing documentation.
```

### Success Criteria

#### Automated Verification:
- [x] Documentation builds without errors
- [x] CI workflow YAML is valid: `yamllint .github/workflows/visual-tests.yml`
- [x] All links in docs are valid
- [x] Code examples compile

#### Manual Verification:
- [ ] Documentation is clear and comprehensive
- [ ] CI workflow runs successfully on push (requires pushing to trigger)
- [ ] Artifacts uploaded correctly (requires CI run)
- [ ] README demos render properly on GitHub

---

## Testing Strategy

### Unit Tests
Not affected - continue using TestBackend for fast logic tests.

### Integration Tests
**Silent PTY Tests** (CI):
- Run with `--ignored` flag
- No visual output
- Fast execution
- Verify workflows complete

**Visual Debug Tests** (Local):
- Run with `--features visual-debug`
- Open terminal window
- Manual observation
- Debug issues

**vt100 Screen Tests** (CI):
- Run with `--features vt100-tests`
- Parse terminal state
- Verify layout accuracy
- Check specific elements

### Manual Testing Steps

1. **Test visual debug mode**:
   ```bash
   cargo build --features visual-debug
   cargo test test_visual_delete -- --ignored --features visual-debug --nocapture
   ```
   - Verify terminal window opens
   - Watch test execute
   - Confirm interactions visible

2. **Test VHS recording**:
   ```bash
   ./scripts/record-demos.sh
   open tests/recordings/delete-session.gif
   ```
   - Verify GIF created
   - Check quality
   - Verify timing

3. **Test vt100 verification**:
   ```bash
   cargo test --features vt100-tests -- --ignored --nocapture
   ```
   - Check screen parsing works
   - Verify assertions pass
   - Review cursor position

---

## Performance Considerations

### Visual Debug Mode
- Slower due to terminal window overhead
- Only use for debugging specific issues
- Not suitable for CI

### VHS Recording
- Heavy - requires building release binary
- Long recording times (real-time capture)
- Only run on-demand or main branch pushes

### vt100 Tests
- Minimal overhead (parsing only)
- Suitable for CI
- Faster than visual debug

---

## Migration Notes

### Existing Tests
No changes required - all existing tests continue working.

### Adding Visual Tests
Follow pattern:
```rust
#[test]
#[ignore]
#[cfg(feature = "visual-debug")]
fn test_visual_something() {
    // Test code
}
```

### Recording Demos
Create tape file, run `vhs tests/tapes/your-demo.tape`.

---

## References

### Documentation
- Existing: `tests/E2E_TESTING.md` - PTY testing guide
- Existing: `tests/e2e_pty_tests.rs:10-34` - spawn_app() helper
- VHS: https://github.com/charmbracelet/vhs
- rexpect: https://github.com/rust-cli/rexpect
- vt100: https://docs.rs/vt100

### Similar Implementations
- Ratatui examples: https://github.com/ratatui/ratatui/tree/main/examples
- Testing TUI Apps: https://blog.waleedkhan.name/testing-tui-apps/

### Dependencies
- rexpect 0.5 (already in Cargo.toml)
- vt100 0.15 (already in Cargo.toml)
- VHS (external tool, brew install)
