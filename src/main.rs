// ABOUTME: Main entry point for Claude-in-a-Box with TUI and CLI support

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::Backend, prelude::*};
use std::{
    io::{self, IsTerminal},
    time::{Duration, Instant},
};

mod app;
mod claude;
mod components;
mod config;
mod docker;
mod git;
mod models;
mod terminal;

use app::{App, EventHandler};
use components::LayoutComponent;

/// Terminal cleanup utility to ensure proper restoration
fn cleanup_terminal() {
    let _ = disable_raw_mode();
    // Use stdout for cleanup since that's where we enabled mouse capture
    let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
}

/// Unified terminal cleanup that works with a terminal instance
fn cleanup_terminal_with_instance<B: Backend + std::io::Write>(
    terminal: &mut Terminal<B>,
) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

#[derive(Parser)]
#[command(name = "claude-box")]
#[command(about = "Terminal-based development environment manager for Claude Code containers")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Set up Claude authentication for containers
    Auth,
}

#[tokio::main]
async fn main() -> Result<()> {
    setup_logging();
    setup_panic_handler();

    let cli = Cli::parse();

    let result = match cli.command {
        Some(Commands::Auth) => run_auth_setup().await,
        None => {
            // No command specified, run TUI
            let mut app = App::new();
            app.init().await;
            let mut layout = LayoutComponent::new();

            run_tui(&mut app, &mut layout).await
        }
    };

    // Ensure terminal is cleaned up on any error
    if result.is_err() {
        cleanup_terminal();
    }

    result
}

async fn run_auth_setup() -> Result<()> {
    println!("🔐 Setting up Claude authentication for claude-in-a-box...");
    println!();

    // Create the auth directory structure
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    let claude_box_dir = home_dir.join(".claude-in-a-box");
    let auth_dir = claude_box_dir.join("auth");

    std::fs::create_dir_all(&auth_dir)
        .map_err(|e| anyhow::anyhow!("Failed to create auth directory: {}", e))?;

    // Check if credentials already exist
    let credentials_path = auth_dir.join(".credentials.json");
    if credentials_path.exists() {
        println!("✅ Authentication already set up!");
        println!("   Credentials found at: {}", credentials_path.display());
        println!();
        println!("To re-authenticate, delete the credentials file and run this command again:");
        println!("   rm {}", credentials_path.display());
        return Ok(());
    }

    println!("📁 Creating auth directories...");
    println!("   Auth directory: {}", auth_dir.display());

    // Check if Docker is available
    let docker_version =
        std::process::Command::new("docker").args(["--version"]).output().map_err(|e| {
            anyhow::anyhow!(
                "Docker not found: {}. Please install Docker and try again.",
                e
            )
        })?;

    if !docker_version.status.success() {
        return Err(anyhow::anyhow!(
            "Docker is not running. Please start Docker and try again."
        ));
    }

    println!("🏗️  Building authentication container (claude-dev)...");
    let build_status = std::process::Command::new("docker")
        .args(["build", "-t", "claude-box:claude-dev", "docker/claude-dev"])
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to build container: {}", e))?;

    if !build_status.success() {
        return Err(anyhow::anyhow!(
            "Container build failed. Please check Docker and try again."
        ));
    }

    // Execute the auth container
    println!();
    println!("🚀 Running authentication setup...");
    println!("   This will prompt you to enter your Anthropic API token.");
    println!();

    let status = std::process::Command::new("docker")
        .args([
            "run",
            "--rm",
            "-it",
            "-v",
            &format!("{}:/home/claude-user/.claude", auth_dir.display()),
            "-e",
            "PATH=/home/claude-user/.npm-global/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            "-e",
            "HOME=/home/claude-user",
            "-w",
            "/home/claude-user",
            "--user",
            "claude-user",
            "--entrypoint",
            "bash",
            "claude-box:claude-dev",
            "-c",
            "/app/scripts/auth-setup.sh",
        ])
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to run auth container: {}", e))?;

    if status.success() {
        println!();
        println!("🎉 Authentication setup complete!");
        println!("   Credentials saved to: {}", credentials_path.display());
        println!();
        println!("You can now create claude-box development sessions with:");
        println!("   claude-box");
    } else {
        println!();
        println!("❌ Authentication setup failed!");
        println!("   Please check the output above for errors and try again.");
        std::process::exit(1);
    }

    Ok(())
}

async fn run_tui(app: &mut App, layout: &mut LayoutComponent) -> Result<()> {
    // Check if we have a proper TTY
    if !IsTerminal::is_terminal(&io::stdout()) {
        return Err(anyhow::anyhow!(
            "No TTY detected. This application requires a terminal.\n\
             Try running directly in a terminal instead of redirecting output."
        ));
    }

    // Check if we're in a proper terminal
    match crossterm::terminal::is_raw_mode_enabled() {
        Ok(false) => {
            // Raw mode is not enabled, which is normal - we'll enable it
        }
        Err(e) => {
            eprintln!("Cannot check terminal raw mode: {}", e);
            return Err(anyhow::anyhow!("Terminal not compatible: {}", e));
        }
        Ok(true) => {
            // Raw mode is already enabled, continue
        }
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Ensure terminal cleanup happens even if there's an error
    let result = run_tui_loop(app, layout, &mut terminal).await;

    // Always clean up terminal using unified cleanup
    if let Err(e) = cleanup_terminal_with_instance(&mut terminal) {
        tracing::error!("Failed to cleanup terminal: {}", e);
        // Fallback to basic cleanup
        cleanup_terminal();
    }

    result
}

async fn run_tui_loop(
    app: &mut App,
    layout: &mut LayoutComponent,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<()> {
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|frame| {
            layout.render(frame, &app.state);
        })?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            match event::read()? {
                Event::Key(key_event) => {
                    // Special handling for interactive terminal
                    use crate::app::state::View;
                    if app.state.current_view == View::AttachedTerminal {
                        // Pass key events directly to the interactive session
                        let continue_session = layout.handle_interactive_session_input(key_event).await;
                        if !continue_session {
                            // User detached from session
                            app.state.attached_session_id = None;
                            app.state.current_view = View::SessionList;
                        }
                    } else if let Some(app_event) =
                        EventHandler::handle_key_event(key_event, &mut app.state)
                    {
                        // Handle scroll events for live logs
                        use crate::app::events::AppEvent;
                        match app_event {
                            AppEvent::ScrollLogsUp => {
                                layout.live_logs_mut().scroll_up();
                            }
                            AppEvent::ScrollLogsDown => {
                                let total_logs =
                                    app.state.live_logs.values().map(|v| v.len()).sum::<usize>();
                                layout.live_logs_mut().scroll_down(total_logs);
                            }
                            AppEvent::ScrollLogsToTop => {
                                layout.live_logs_mut().scroll_to_top();
                            }
                            AppEvent::ScrollLogsToBottom => {
                                let total_logs =
                                    app.state.live_logs.values().map(|v| v.len()).sum::<usize>();
                                layout.live_logs_mut().scroll_to_bottom(total_logs);
                            }
                            _ => {
                                // Process other events normally
                                EventHandler::process_event(app_event, &mut app.state);
                            }
                        }
                    }
                }
                Event::Mouse(_) => {}
                Event::Resize(_, _) => {}
                Event::FocusGained => {}
                Event::FocusLost => {}
                Event::Paste(_) => {}
            }
        }

        // Process any pending events
        if let Some(pending_event) = app.state.pending_event.take() {
            EventHandler::process_event(pending_event, &mut app.state);
        }

        if last_tick.elapsed() >= tick_rate {
            match app.tick().await {
                Ok(()) => {
                    last_tick = Instant::now();

                    // Check if UI needs immediate refresh after async operations
                    if app.needs_ui_refresh() {
                        // Force immediate redraw by skipping the timeout
                        terminal.draw(|frame| {
                            layout.render(frame, &app.state);
                        })?;
                    }
                }
                Err(e) => {
                    use tracing::error;
                    error!("Error during app tick: {}", e);
                    // Continue running instead of crashing
                    last_tick = Instant::now();
                }
            }
        }

        if app.state.should_quit {
            break;
        }
    }

    Ok(())
}

fn setup_logging() {
    use std::fs::OpenOptions;
    use std::path::PathBuf;
    use tracing_subscriber::prelude::*;

    // Create log directory if it doesn't exist
    let log_dir = std::env::var("HOME")
        .map(|home| PathBuf::from(home).join(".claude-in-a-box").join("logs"))
        .unwrap_or_else(|_| PathBuf::from(".claude-in-a-box/logs"));

    let _ = std::fs::create_dir_all(&log_dir);

    // Create log file with timestamp
    let log_file = log_dir.join(format!(
        "claude-in-a-box-{}.log",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));

    // Open file for writing
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .expect("Failed to create log file");

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_writer(file)
                .with_ansi(false), // No ANSI colors in log file
        )
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "claude_box=info".into()),
        )
        .init();
}

fn setup_panic_handler() {
    use tracing::error;

    std::panic::set_hook(Box::new(|panic_info| {
        // Ensure terminal is restored before logging the panic
        cleanup_terminal();

        error!("Application panicked: {}", panic_info);
        eprintln!("Application panicked: {}", panic_info);
        eprintln!("Please check the logs for more details.");
    }));
}
