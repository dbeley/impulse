mod browser;
mod config;
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
    // Load configuration
    let config = match config::Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error loading configuration: {}", e);
            std::process::exit(1);
        }
    };

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
