mod browser;
mod config;
mod lastfm;
mod lastfm_auth;
mod logger;
mod metadata;
mod player;
mod playlist;
mod queue;
mod ui;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Load songs from text file(s) with "artist - song" format (one per line). Can be specified multiple times.
    #[arg(short, long, value_name = "FILE")]
    load_playlist: Vec<PathBuf>,
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

    // Initialize logger
    if let Err(e) = logger::init_logger(config.log_file.clone()) {
        eprintln!("Warning: Failed to initialize logger: {}", e);
    } else {
        logger::log("Impulse music player started");
    }

    if let Err(e) = lastfm_auth::ensure_lastfm_session_key(&mut config) {
        eprintln!("Warning: {}", e);
        eprintln!("Last.fm scrobbling will remain disabled until the session key is configured.");
    }

    // Load songs from file(s) if provided
    let initial_queue = if !args.load_playlist.is_empty() {
        let mut all_tracks = Vec::new();
        for playlist_file in &args.load_playlist {
            logger::log(&format!("Loading playlist: {}", playlist_file.display()));
            match load_songs_from_file(playlist_file, &config.music_dir) {
                Ok(tracks) => {
                    if tracks.is_empty() {
                        let msg = format!(
                            "Warning: No matching songs found from {}",
                            playlist_file.display()
                        );
                        eprintln!("{}", msg);
                        logger::log(&msg);
                    } else {
                        let msg = format!(
                            "Loaded {} songs from {}",
                            tracks.len(),
                            playlist_file.display()
                        );
                        eprintln!("{}", msg);
                        logger::log(&msg);
                        all_tracks.extend(tracks);
                    }
                }
                Err(e) => {
                    let msg = format!(
                        "Error loading songs from {}: {}",
                        playlist_file.display(),
                        e
                    );
                    eprintln!("{}", msg);
                    logger::log(&msg);
                }
            }
        }
        all_tracks
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
        } else {
            logger::log(&format!("Track not found: {}", query));
        }
    }

    Ok(found_tracks)
}

fn normalize_for_matching(s: &str) -> String {
    s.to_lowercase()
        .replace(
            &['\u{2019}', '\'', '\u{201c}', '\u{201d}', '"', '`'][..],
            "",
        )
        .replace('&', "and")
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

fn find_matching_song_optimized(music_dir: &Path, query: &str) -> Option<PathBuf> {
    // Parse "Artist - Song" format
    let parts: Vec<&str> = query.split('-').map(|s| s.trim()).collect();
    if parts.is_empty() {
        return None;
    }

    let artist = normalize_for_matching(parts[0]);
    let song = if parts.len() > 1 {
        normalize_for_matching(&parts[1..].join("-"))
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
                    let dir_name_normalized = normalize_for_matching(dir_name);

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
                    let filename_normalized = normalize_for_matching(filename);

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_music_library(temp_dir: &Path) -> PathBuf {
        let music_dir = temp_dir.join("music");
        fs::create_dir_all(&music_dir).unwrap();

        // Create artist directories with songs
        let artist1 = music_dir.join("Artist One");
        fs::create_dir_all(&artist1).unwrap();
        fs::write(artist1.join("song1.mp3"), "dummy audio").unwrap();
        fs::write(artist1.join("song2.mp3"), "dummy audio").unwrap();

        let artist2 = music_dir.join("Artist Two");
        fs::create_dir_all(&artist2).unwrap();
        fs::write(artist2.join("track one.mp3"), "dummy audio").unwrap();

        let artist3 = music_dir.join("The Third Artist");
        let album = artist3.join("Album Name");
        fs::create_dir_all(&album).unwrap();
        fs::write(album.join("best song.mp3"), "dummy audio").unwrap();

        music_dir
    }

    #[test]
    fn test_load_songs_from_file_single_file() {
        let temp_dir = TempDir::new().unwrap();
        let music_dir = create_test_music_library(temp_dir.path());

        let playlist_file = temp_dir.path().join("playlist.txt");
        fs::write(
            &playlist_file,
            "Artist One - song1\nArtist Two - track one\n",
        )
        .unwrap();

        let result = load_songs_from_file(&playlist_file, &music_dir).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_load_songs_from_file_with_numbers() {
        let temp_dir = TempDir::new().unwrap();
        let music_dir = create_test_music_library(temp_dir.path());

        let playlist_file = temp_dir.path().join("playlist.txt");
        fs::write(
            &playlist_file,
            "1. Artist One - song1\n2. Artist Two - track one\n3. The Third Artist - best song\n",
        )
        .unwrap();

        let result = load_songs_from_file(&playlist_file, &music_dir).unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_load_songs_from_file_empty_lines() {
        let temp_dir = TempDir::new().unwrap();
        let music_dir = create_test_music_library(temp_dir.path());

        let playlist_file = temp_dir.path().join("playlist.txt");
        fs::write(
            &playlist_file,
            "\nArtist One - song1\n\n\nArtist Two - track one\n\n",
        )
        .unwrap();

        let result = load_songs_from_file(&playlist_file, &music_dir).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_load_songs_from_file_nonexistent_songs() {
        let temp_dir = TempDir::new().unwrap();
        let music_dir = create_test_music_library(temp_dir.path());

        let playlist_file = temp_dir.path().join("playlist.txt");
        fs::write(
            &playlist_file,
            "Artist One - song1\nNonexistent Artist - fake song\nArtist Two - track one\n",
        )
        .unwrap();

        let result = load_songs_from_file(&playlist_file, &music_dir).unwrap();
        // Should find 2 out of 3 songs (the middle one doesn't exist)
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_load_songs_from_multiple_files() {
        let temp_dir = TempDir::new().unwrap();
        let music_dir = create_test_music_library(temp_dir.path());

        let playlist1 = temp_dir.path().join("playlist1.txt");
        fs::write(&playlist1, "Artist One - song1\n").unwrap();

        let playlist2 = temp_dir.path().join("playlist2.txt");
        fs::write(&playlist2, "Artist Two - track one\n").unwrap();

        let playlist3 = temp_dir.path().join("playlist3.txt");
        fs::write(&playlist3, "The Third Artist - best song\n").unwrap();

        // Simulate loading from multiple files
        let mut all_tracks = Vec::new();
        all_tracks.extend(load_songs_from_file(&playlist1, &music_dir).unwrap());
        all_tracks.extend(load_songs_from_file(&playlist2, &music_dir).unwrap());
        all_tracks.extend(load_songs_from_file(&playlist3, &music_dir).unwrap());

        assert_eq!(all_tracks.len(), 3);
    }

    #[test]
    fn test_load_songs_sequential_order() {
        let temp_dir = TempDir::new().unwrap();
        let music_dir = create_test_music_library(temp_dir.path());

        let playlist_file = temp_dir.path().join("playlist.txt");
        fs::write(
            &playlist_file,
            "Artist Two - track one\nArtist One - song1\nArtist One - song2\n",
        )
        .unwrap();

        let result = load_songs_from_file(&playlist_file, &music_dir).unwrap();
        assert_eq!(result.len(), 3);

        // Verify order is preserved
        assert!(result[0].to_string_lossy().contains("track one"));
        assert!(result[1].to_string_lossy().contains("song1"));
        assert!(result[2].to_string_lossy().contains("song2"));
    }

    #[test]
    fn test_find_matching_song_no_dash() {
        let temp_dir = TempDir::new().unwrap();
        let music_dir = create_test_music_library(temp_dir.path());

        // Query without dash should return None
        let result = find_matching_song_optimized(&music_dir, "Just A Song Name");
        assert!(result.is_none());
    }

    #[test]
    fn test_find_matching_song_in_subdirectory() {
        let temp_dir = TempDir::new().unwrap();
        let music_dir = create_test_music_library(temp_dir.path());

        // Song in album subdirectory
        let result = find_matching_song_optimized(&music_dir, "The Third Artist - best song");
        assert!(result.is_some());
        assert!(result.unwrap().to_string_lossy().contains("best song"));
    }

    #[test]
    fn test_normalize_for_matching_lowercase() {
        assert_eq!(normalize_for_matching("HELLO WORLD"), "hello world");
        assert_eq!(normalize_for_matching("MiXeD CaSe"), "mixed case");
    }

    #[test]
    fn test_normalize_for_matching_quotes() {
        assert_eq!(normalize_for_matching("don't"), "dont");
        assert_eq!(normalize_for_matching("don't"), "dont");
        assert_eq!(normalize_for_matching("say \"hello\""), "say hello");
        assert_eq!(normalize_for_matching("\u{201c}quoted\u{201d}"), "quoted");
        assert_eq!(
            normalize_for_matching("\u{201c}also quoted\u{201d}"),
            "also quoted"
        );
        assert_eq!(normalize_for_matching("`backtick`"), "backtick");
    }

    #[test]
    fn test_normalize_for_matching_ampersand() {
        assert_eq!(normalize_for_matching("rock & roll"), "rock and roll");
        assert_eq!(normalize_for_matching("R&B"), "randb");
        assert_eq!(normalize_for_matching("A & B & C"), "a and b and c");
    }

    #[test]
    fn test_normalize_for_matching_special_chars() {
        assert_eq!(normalize_for_matching("hello@world"), "helloworld");
        assert_eq!(normalize_for_matching("test#123"), "test123");
        assert_eq!(normalize_for_matching("a/b/c"), "abc");
        assert_eq!(normalize_for_matching("hello!world?"), "helloworld");
    }

    #[test]
    fn test_normalize_for_matching_whitespace() {
        assert_eq!(normalize_for_matching("hello   world"), "hello world");
        assert_eq!(normalize_for_matching("  trim  me  "), "trim me");
        assert_eq!(normalize_for_matching("a\tb\nc"), "a b c");
    }

    #[test]
    fn test_normalize_for_matching_alphanumeric() {
        assert_eq!(normalize_for_matching("test123"), "test123");
        assert_eq!(normalize_for_matching("abc 123 xyz 789"), "abc 123 xyz 789");
    }

    #[test]
    fn test_normalize_for_matching_combined() {
        assert_eq!(
            normalize_for_matching("The Beatles' \"Hey Jude\" & More"),
            "the beatles hey jude and more"
        );
        assert_eq!(
            normalize_for_matching("AC/DC - Back in Black (Live)"),
            "acdc back in black live"
        );
        assert_eq!(
            normalize_for_matching("It's a   Beautiful  Day!"),
            "its a beautiful day"
        );
    }

    #[test]
    fn test_normalize_for_matching_empty() {
        assert_eq!(normalize_for_matching(""), "");
        assert_eq!(normalize_for_matching("   "), "");
        assert_eq!(normalize_for_matching("@#$%"), "");
    }

    #[test]
    fn test_normalize_for_matching_unicode() {
        assert_eq!(normalize_for_matching("café"), "caf");
        assert_eq!(normalize_for_matching("naïve"), "nave");
        assert_eq!(normalize_for_matching("Björk"), "bjrk");
    }
}
