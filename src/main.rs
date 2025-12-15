mod browser;
mod config;
mod lastfm;
mod lastfm_auth;
mod metadata;
mod player;
mod playlist;
mod queue;
mod ui;

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

fn main() -> Result<()> {
    // Load and potentially augment configuration
    let mut config = match config::Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error loading configuration: {}", e);
            std::process::exit(1);
        }
    };
    if let Err(e) = lastfm_auth::ensure_lastfm_session_key(&mut config) {
        eprintln!("Warning: {}", e);
        eprintln!("Last.fm scrobbling will remain disabled until the session key is configured.");
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create and run app
    let mut app = match ui::App::new(config) {
        Ok(app) => app,
        Err(e) => {
            // Restore terminal before showing error
            disable_raw_mode()?;
            execute!(io::stdout(), LeaveAlternateScreen)?;
            eprintln!("\nError initializing audio player: {}", e);
            eprintln!("Make sure you have audio devices configured on your system.");
            eprintln!("\nFor headless systems or systems without audio:");
            eprintln!("  - Install a virtual audio device like 'pulseaudio' or 'alsa-utils'");
            eprintln!("  - Configure a dummy audio output");
            std::process::exit(1);
        }
    };

    let res = app.run(&mut terminal);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}
