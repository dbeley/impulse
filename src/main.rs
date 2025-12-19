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
use clap::Parser;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Load songs from a text file with "artist - song" format (one per line)
    #[arg(short, long, value_name = "FILE")]
    load_playlist: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
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

    // Load songs from file if provided
    let initial_queue = if let Some(playlist_file) = args.load_playlist {
        match load_songs_from_file(&playlist_file, &config.music_dir) {
            Ok(tracks) => {
                if tracks.is_empty() {
                    eprintln!(
                        "Warning: No matching songs found from {}",
                        playlist_file.display()
                    );
                } else {
                    eprintln!(
                        "Loaded {} songs from {}",
                        tracks.len(),
                        playlist_file.display()
                    );
                }
                tracks
            }
            Err(e) => {
                eprintln!("Error loading songs from file: {}", e);
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create and run app
    let mut app = match ui::App::new(config) {
        Ok(mut app) => {
            // Add initial queue songs if any
            if !initial_queue.is_empty() {
                app.load_initial_queue(initial_queue);
            }
            app
        }
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

fn load_songs_from_file(playlist_file: &Path, music_dir: &Path) -> Result<Vec<PathBuf>> {
    let content = fs::read_to_string(playlist_file)?;
    let mut found_tracks = Vec::new();

    // Parse each line and search for matching files
    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines
        if line.is_empty() {
            continue;
        }

        // Remove leading numbers and dots if present
        let query =
            line.trim_start_matches(|c: char| c.is_numeric() || c == '.' || c.is_whitespace());

        if query.is_empty() {
            continue;
        }

        // Try to find a matching file
        if let Some(matched_path) = find_matching_song_optimized(music_dir, query) {
            found_tracks.push(matched_path);
        }
    }

    Ok(found_tracks)
}

fn find_matching_song_optimized(music_dir: &Path, query: &str) -> Option<PathBuf> {
    // Parse "Artist - Song" format
    let parts: Vec<&str> = query.split('-').map(|s| s.trim()).collect();
    if parts.is_empty() {
        return None;
    }

    let artist = parts[0].to_lowercase();
    let song = if parts.len() > 1 {
        parts[1..].join("-").to_lowercase()
    } else {
        return None; // Need both artist and song
    };

    // Step 1: Find candidate artist directories
    let mut candidate_dirs = Vec::new();

    if let Ok(entries) = fs::read_dir(music_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                    let dir_name_normalized = dir_name.to_lowercase();

                    // Check if directory name contains artist name
                    if dir_name_normalized.contains(&artist) {
                        candidate_dirs.push(path);
                    }
                }
            }
        }
    }

    // Step 2: Search for song in candidate directories
    let mut best_match: Option<(PathBuf, i64)> = None;

    for artist_dir in candidate_dirs {
        // Search up to 2 levels deep in artist directory (for album subdirectories)
        for entry in WalkDir::new(&artist_dir)
            .max_depth(2)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && browser::is_audio_file(path) {
                if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                    let filename_normalized = filename.to_lowercase();

                    // Calculate match score based on song title
                    if filename_normalized.contains(&song) {
                        // Prefer exact matches
                        let score = if filename_normalized == song {
                            10000 + song.len() as i64
                        } else {
                            song.len() as i64
                        };

                        if let Some((_, best_score)) = &best_match {
                            if score > *best_score {
                                best_match = Some((path.to_path_buf(), score));
                            }
                        } else {
                            best_match = Some((path.to_path_buf(), score));
                        }
                    }
                }
            }
        }

        // If we found a match in this artist directory, return it
        // This prevents scanning other artist directories unnecessarily
        if best_match.is_some() {
            break;
        }
    }

    best_match.map(|(path, _)| path)
}
