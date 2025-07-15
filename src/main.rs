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
mod git;
mod models;

use app::{App, EventHandler};
use components::LayoutComponent;

#[tokio::main]
async fn main() -> Result<()> {
    setup_logging();
    
    let mut app = App::new();
    let mut layout = LayoutComponent::new();
    
    run_tui(&mut app, &mut layout).await?;
    
    Ok(())
}

async fn run_tui(app: &mut App, layout: &mut LayoutComponent) -> Result<()> {
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
            app.tick();
            last_tick = Instant::now();
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
    tracing_subscriber::fmt()
        .with_env_filter("claude_box=debug")
        .init();
}
