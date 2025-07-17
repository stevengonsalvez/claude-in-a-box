// ABOUTME: Main entry point for Claude-in-a-Box TUI application

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    Terminal,
};
use std::{
    io,
    time::{Duration, Instant},
};

mod app;
mod components;
mod config;
mod docker;
mod git;
mod models;

use app::{App, EventHandler};
use components::LayoutComponent;

#[tokio::main]
async fn main() -> Result<()> {
    setup_logging();
    setup_panic_handler();
    
    let mut app = App::new();
    app.init().await;
    let mut layout = LayoutComponent::new();
    
    run_tui(&mut app, &mut layout).await?;
    
    Ok(())
}

async fn run_tui(app: &mut App, layout: &mut LayoutComponent) -> Result<()> {
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
                    if let Some(app_event) = EventHandler::handle_key_event(key_event, &mut app.state) {
                        EventHandler::process_event(app_event, &mut app.state);
                    }
                }
                Event::Mouse(_) => {}
                Event::Resize(_, _) => {}
                Event::FocusGained => {}
                Event::FocusLost => {}
                Event::Paste(_) => {}
            }
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

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn setup_logging() {
    use tracing_subscriber::prelude::*;
    use std::fs::OpenOptions;
    use std::path::PathBuf;
    
    // Create log directory if it doesn't exist
    let log_dir = std::env::var("HOME")
        .map(|home| PathBuf::from(home).join(".claude-in-a-box").join("logs"))
        .unwrap_or_else(|_| PathBuf::from(".claude-in-a-box/logs"));
    
    let _ = std::fs::create_dir_all(&log_dir);
    
    // Create log file with timestamp
    let log_file = log_dir.join(format!("claude-in-a-box-{}.log", 
        chrono::Local::now().format("%Y%m%d-%H%M%S")));
    
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
                .with_ansi(false) // No ANSI colors in log file
        )
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "claude_box=info".into())
        )
        .init();
}

fn setup_panic_handler() {
    use tracing::error;
    
    std::panic::set_hook(Box::new(|panic_info| {
        // Ensure terminal is restored before logging the panic
        let _ = disable_raw_mode();
        let _ = execute!(
            std::io::stderr(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        
        error!("Application panicked: {}", panic_info);
        eprintln!("Application panicked: {}", panic_info);
        eprintln!("Please check the logs for more details.");
    }));
}
