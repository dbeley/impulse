use crate::browser::Browser;
use crate::config::Config;
use crate::lastfm::LastfmScrobbler;
use crate::player::Player;
use crate::playlist::PlaylistManager;
use crate::queue::Queue;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::{
    Frame, Terminal,
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tab {
    Browser,
    NowPlaying,
    Playlists,
}

impl Tab {
    fn next(&self) -> Self {
        match self {
            Tab::Browser => Tab::NowPlaying,
            Tab::NowPlaying => Tab::Playlists,
            Tab::Playlists => Tab::Browser,
        }
    }

    fn prev(&self) -> Self {
        match self {
            Tab::Browser => Tab::Playlists,
            Tab::NowPlaying => Tab::Browser,
            Tab::Playlists => Tab::NowPlaying,
        }
    }

    fn name(&self) -> &str {
        match self {
            Tab::Browser => "1. Browser",
            Tab::NowPlaying => "2. Now Playing",
            Tab::Playlists => "3. Playlists",
        }
    }

    fn help_text(&self) -> &str {
        match self {
            Tab::Browser => {
                "Keys: j/k/↑/↓=nav, l/→/Enter=select, h/←=back, a=add, A=add-all, Space/p=play/pause, >=next, <=prev, r=random, Tab/1-3=switch-tab, /=search, q=quit"
            }
            Tab::NowPlaying => {
                "Keys: j/k/↑/↓=nav, Enter=jump, ←/→=seek, d=delete, K/J=move, c=clear, S=save-queue, Space/p=play/pause, >=next, <=prev, r=random, Tab/1-3=switch-tab, /=search, q=quit"
            }
            Tab::Playlists => {
                "Keys: j/k/↑/↓=nav, l/Enter=add-to-queue, Space/p=play/pause, >=next, <=prev, Tab/1-3=switch-tab, /=search, q=quit"
            }
        }
    }
}

pub enum InputMode {
    Normal,
    Search,
    Command,
}

struct SearchResult {
    path: PathBuf,
    name: String,
    score: i64,
}

pub struct App {
    player: Player,
    browser: Browser,
    queue: Queue,
    playlist_manager: PlaylistManager,
    config: Config,
    current_tab: Tab,
    input_mode: InputMode,
    search_query: String,
    command_input: String,
    search_results: Vec<SearchResult>,
    search_result_selected: usize,
    queue_selected: usize,
    playlist_selected: usize,
    playlist_track_selected: usize,
    should_quit: bool,
    status_message: String,
    status_message_time: Option<SystemTime>,
    image_picker: Arc<Mutex<Picker>>,
    album_art: Arc<Mutex<Option<Box<dyn StatefulProtocol>>>>,
    last_album_art_track: Arc<Mutex<Option<PathBuf>>>,
    lastfm_scrobbler: LastfmScrobbler,
    track_play_time: Arc<Mutex<Option<SystemTime>>>,
    browser_state: ListState,
    queue_state: ListState,
    playlist_state: ListState,
    search_state: ListState,
}

impl App {
    pub fn new(config: Config) -> Result<Self> {
        let player = Player::new()?;
        player.set_volume(config.volume);

        let browser = Browser::new(config.music_dir.clone());
        let queue = Queue::load().unwrap_or_else(|_| Queue::new());
        let playlist_manager = PlaylistManager::new(config.playlist_dir.clone());

        // Initialize image picker for album art display
        let mut picker = Picker::new((8, 12));
        picker.guess_protocol();

        // Initialize Last.fm scrobbler
        let lastfm_scrobbler = LastfmScrobbler::new(config.lastfm.as_ref());

        // If queue was loaded from JSON and has tracks, load the current track but start paused
        if !queue.is_empty() {
            if let Some(track) = queue.current() {
                let _ = player.play(track.clone());
                player.pause();
            }
        }

        Ok(Self {
            player,
            browser,
            queue,
            playlist_manager,
            config,
            current_tab: Tab::Browser,
            input_mode: InputMode::Normal,
            search_query: String::new(),
            command_input: String::new(),
            search_results: Vec::new(),
            search_result_selected: 0,
            queue_selected: 0,
            playlist_selected: 0,
            playlist_track_selected: 0,
            should_quit: false,
            status_message: String::new(),
            status_message_time: None,
            image_picker: Arc::new(Mutex::new(picker)),
            album_art: Arc::new(Mutex::new(None)),
            last_album_art_track: Arc::new(Mutex::new(None)),
            lastfm_scrobbler,
            track_play_time: Arc::new(Mutex::new(None)),
            browser_state: ListState::default(),
            queue_state: ListState::default(),
            playlist_state: ListState::default(),
            search_state: ListState::default(),
        })
    }

    pub fn load_initial_queue(&mut self, tracks: Vec<PathBuf>) {
        for track in tracks {
            self.queue.add(track);
        }
        // Switch to Now Playing tab but start paused
        if !self.queue.is_empty() {
            self.current_tab = Tab::NowPlaying;
            if let Some(track) = self.queue.current() {
                let _ = self.player.play(track.clone());
                self.player.pause();
                self.track_play_time = Arc::new(Mutex::new(Some(SystemTime::now())));
                // Update Last.fm now playing if enabled
                if self.lastfm_scrobbler.is_enabled() {
                    if let Some(metadata) = self.player.current_metadata() {
                        let _ = self.lastfm_scrobbler.now_playing(track, &metadata);
                    }
                }
            }
        }
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            terminal.draw(|f| self.draw(f))?;

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key)?;
                }
            }

            // Clear status message after 3 seconds
            if let Some(message_time) = self.status_message_time {
                if let Ok(elapsed) = SystemTime::now().duration_since(message_time) {
                    if elapsed.as_secs() >= 3 {
                        self.status_message.clear();
                        self.status_message_time = None;
                    }
                }
            }

            // Check if current track finished
            if self.player.is_finished() && !self.queue.is_empty() {
                // Scrobble the finished track if enough time has passed
                self.scrobble_if_needed();
                self.play_next();
            } else if self.player.is_finished() && self.player.current_track().is_some() {
                // Track finished but queue is empty, scrobble and stop player
                self.scrobble_if_needed();
                self.player.stop();
            }

            if self.should_quit {
                // Save queue before quitting
                let _ = self.queue.save();
                break;
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.input_mode {
            InputMode::Normal => self.handle_normal_mode(key)?,
            InputMode::Search => self.handle_search_mode(key)?,
            InputMode::Command => self.handle_command_mode(key)?,
        }
        Ok(())
    }

    fn set_status(&mut self, message: String) {
        self.status_message = message;
        self.status_message_time = Some(SystemTime::now());
    }

    fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Char('?') => {
                self.set_status(self.current_tab.help_text().to_string());
            }
            KeyCode::Char('1') => {
                self.current_tab = Tab::Browser;
            }
            KeyCode::Char('2') => {
                self.current_tab = Tab::NowPlaying;
            }
            KeyCode::Char('3') => {
                self.current_tab = Tab::Playlists;
            }
            KeyCode::Tab => {
                self.current_tab = self.current_tab.next();
            }
            KeyCode::BackTab => {
                self.current_tab = self.current_tab.prev();
            }
            KeyCode::Char('/') => {
                self.input_mode = InputMode::Search;
                self.search_query.clear();
            }
            KeyCode::Char(':') => {
                self.input_mode = InputMode::Command;
                self.command_input.clear();
            }
            KeyCode::Char(' ' | 'p') => {
                if self.player.is_playing() {
                    self.player.pause();
                    self.set_status(String::from("Paused"));
                } else if self.player.is_paused() {
                    self.player.resume();
                    self.set_status(String::from("Resumed"));
                } else if let Some(track) = self.queue.current() {
                    let track_clone = track.clone();
                    let display_path = track_clone.display().to_string();
                    if let Err(e) = self.player.play(track_clone.clone()) {
                        self.set_status(format!("Error playing: {}", e));
                    } else {
                        self.set_status(format!("Playing: {}", display_path));
                        self.start_track(&track_clone);
                    }
                }
            }
            KeyCode::Char('>') => {
                self.play_next();
            }
            KeyCode::Char('<') => {
                self.play_prev();
            }
            KeyCode::Char('r') => {
                self.queue.toggle_random();
                let status = if self.queue.is_random() {
                    "Random mode enabled"
                } else {
                    "Random mode disabled"
                };
                self.set_status(String::from(status));
            }
            KeyCode::Char('s') => {
                self.scrobble_if_needed();
                self.player.stop();
                self.set_status(String::from("Stopped"));
            }
            _ => {
                self.handle_tab_keys(key)?;
            }
        }
        Ok(())
    }

    fn handle_tab_keys(&mut self, key: KeyEvent) -> Result<()> {
        match self.current_tab {
            Tab::Browser => self.handle_browser_keys(key)?,
            Tab::NowPlaying => self.handle_now_playing_keys(key)?,
            Tab::Playlists => self.handle_playlist_keys(key)?,
        }
        Ok(())
    }

    fn handle_now_playing_keys(&mut self, key: KeyEvent) -> Result<()> {
        // Handle seek keys first (specific to now playing)
        match key.code {
            KeyCode::Left => {
                self.player.seek_backward(5);
                self.set_status(String::from("Seeked backward 5s"));
                return Ok(());
            }
            KeyCode::Right => {
                self.player.seek_forward(5);
                self.set_status(String::from("Seeked forward 5s"));
                return Ok(());
            }
            _ => {}
        }

        // Allow queue navigation/manipulation and player controls in the combined view
        self.handle_queue_keys(key)?;
        self.handle_player_keys(key)?;
        Ok(())
    }

    fn handle_browser_keys(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.browser.select_next();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.browser.select_prev();
            }
            KeyCode::Char('g') => {
                self.browser.select_first();
            }
            KeyCode::Char('G') => {
                self.browser.select_last();
            }
            KeyCode::PageDown => {
                for _ in 0..10 {
                    self.browser.select_next();
                }
            }
            KeyCode::PageUp => {
                for _ in 0..10 {
                    self.browser.select_prev();
                }
            }
            KeyCode::Char('l') | KeyCode::Enter | KeyCode::Right => {
                if let Some(track) = self.browser.enter_selected() {
                    let was_empty = self.queue.is_empty();
                    self.queue.add(track.clone());
                    self.set_status(format!("Added to queue: {}", track.display()));

                    if was_empty && !self.player.is_playing() {
                        if let Err(e) = self.player.play(track.clone()) {
                            self.set_status(format!("Error playing: {}", e));
                        } else {
                            self.player.pause();
                            self.start_track(&track);
                        }
                    }
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.browser.go_parent();
            }
            KeyCode::Char('a') => {
                if let Some(entry) = self.browser.selected_entry() {
                    if !entry.is_dir() {
                        self.queue.add(entry.path().to_path_buf());
                        self.set_status(format!("Added to queue: {}", entry.name()));
                    }
                }
            }
            KeyCode::Char('A') => {
                let files = self.browser.get_all_audio_files();
                let count = files.len();
                self.queue.add_multiple(files);
                self.set_status(format!("Added {} tracks to queue", count));
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_queue_keys(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.queue.is_empty() {
                    self.queue_selected = (self.queue_selected + 1).min(self.queue.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.queue_selected = self.queue_selected.saturating_sub(1);
            }
            KeyCode::Char('g') => {
                self.queue_selected = 0;
            }
            KeyCode::Char('G') => {
                if !self.queue.is_empty() {
                    self.queue_selected = self.queue.len() - 1;
                }
            }
            KeyCode::PageDown => {
                if !self.queue.is_empty() {
                    self.queue_selected = (self.queue_selected + 10).min(self.queue.len() - 1);
                }
            }
            KeyCode::PageUp => {
                self.queue_selected = self.queue_selected.saturating_sub(10);
            }
            KeyCode::Enter => {
                // Scrobble current track if needed before jumping
                self.scrobble_if_needed();

                if let Some(track) = self.queue.jump_to(self.queue_selected) {
                    let track_clone = track.clone();
                    let display_path = track_clone.display().to_string();
                    if let Err(e) = self.player.play(track_clone.clone()) {
                        self.set_status(format!("Error playing: {}", e));
                    } else {
                        self.set_status(format!("Playing: {}", display_path));
                        self.start_track(&track_clone);
                    }
                }
            }
            KeyCode::Char('d') | KeyCode::Delete | KeyCode::Backspace => {
                if !self.queue.is_empty() {
                    self.queue.remove(self.queue_selected);
                    self.set_status(String::from("Removed from queue"));
                    if self.queue_selected >= self.queue.len() && !self.queue.is_empty() {
                        self.queue_selected = self.queue.len() - 1;
                    }
                }
            }
            KeyCode::Char('K') => {
                if self.queue_selected > 0 {
                    self.queue.move_up(self.queue_selected);
                    self.queue_selected -= 1;
                    self.set_status(String::from("Moved track up"));
                }
            }
            KeyCode::Char('J') => {
                if self.queue_selected + 1 < self.queue.len() {
                    self.queue.move_down(self.queue_selected);
                    self.queue_selected += 1;
                    self.set_status(String::from("Moved track down"));
                }
            }
            KeyCode::Char('c') => {
                self.queue.clear();
                self.queue_selected = 0;
                self.set_status(String::from("Queue cleared"));
            }
            KeyCode::Char('S') => {
                if self.queue.is_empty() {
                    self.set_status(String::from("Queue is empty - nothing to save"));
                } else {
                    self.input_mode = InputMode::Command;
                    self.command_input = String::from("save-queue ");
                    self.set_status(String::from(
                        "Enter name to save queue as playlist (default folder)",
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_player_keys(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('+' | '=') => {
                self.config.volume = (self.config.volume + 0.1).min(1.0);
                self.player.set_volume(self.config.volume);
                self.set_status(format!("Volume: {:.0}%", self.config.volume * 100.0));
            }
            KeyCode::Char('-') => {
                self.config.volume = (self.config.volume - 0.1).max(0.0);
                self.player.set_volume(self.config.volume);
                self.set_status(format!("Volume: {:.0}%", self.config.volume * 100.0));
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_playlist_keys(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                let playlists = self.playlist_manager.playlists();
                if !playlists.is_empty() {
                    self.playlist_selected = (self.playlist_selected + 1).min(playlists.len() - 1);
                    self.playlist_track_selected = 0;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.playlist_selected = self.playlist_selected.saturating_sub(1);
                self.playlist_track_selected = 0;
            }
            KeyCode::Char('g') => {
                self.playlist_selected = 0;
                self.playlist_track_selected = 0;
            }
            KeyCode::Char('G') => {
                let playlists = self.playlist_manager.playlists();
                if !playlists.is_empty() {
                    self.playlist_selected = playlists.len() - 1;
                    self.playlist_track_selected = 0;
                }
            }
            KeyCode::PageDown => {
                let playlists = self.playlist_manager.playlists();
                if !playlists.is_empty() {
                    self.playlist_selected = (self.playlist_selected + 10).min(playlists.len() - 1);
                    self.playlist_track_selected = 0;
                }
            }
            KeyCode::PageUp => {
                self.playlist_selected = self.playlist_selected.saturating_sub(10);
                self.playlist_track_selected = 0;
            }
            KeyCode::Char('l') | KeyCode::Enter => {
                if let Some(playlist) = self.playlist_manager.get_playlist(self.playlist_selected) {
                    self.queue.add_multiple(playlist.tracks.clone());
                    self.set_status(format!("Added playlist '{}' to queue", playlist.name));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_search_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Enter => {
                if !self.search_results.is_empty() {
                    if let Some(result) = self.search_results.get(self.search_result_selected) {
                        // Select the entry in the browser
                        self.browser.select_entry_by_path(&result.path);

                        // If it's a directory, we could enter it, but for now just select it
                        // If it's a file, just select it
                        self.status_message = format!("Selected: {}", result.name);
                    }
                    self.input_mode = InputMode::Normal;
                    self.clear_search_results();
                } else {
                    self.status_message = format!("No match found for: {}", self.search_query);
                    self.input_mode = InputMode::Normal;
                }
            }
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.search_query.clear();
                self.clear_search_results();
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.update_search_results();
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.update_search_results();
            }
            KeyCode::Down => {
                if !self.search_results.is_empty() {
                    self.search_result_selected =
                        (self.search_result_selected + 1).min(self.search_results.len() - 1);
                }
            }
            KeyCode::Up => {
                self.search_result_selected = self.search_result_selected.saturating_sub(1);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_command_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Enter => {
                self.input_mode = InputMode::Normal;
                self.execute_command()?;
            }
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.command_input.clear();
            }
            KeyCode::Char(c) => {
                self.command_input.push(c);
            }
            KeyCode::Backspace => {
                self.command_input.pop();
            }
            _ => {}
        }
        Ok(())
    }

    fn update_search_results(&mut self) {
        let query = self.search_query.trim();
        if query.is_empty() {
            self.search_results.clear();
            self.search_result_selected = 0;
            return;
        }

        let matcher = SkimMatcherV2::default();
        let mut results = Vec::new();

        // Search only in current directory (folders and audio files)
        let entries = self.browser.entries();

        for entry in entries {
            // Skip parent directory entry
            if matches!(entry, crate::browser::FileEntry::ParentDirectory(_)) {
                continue;
            }

            let path = entry.path();
            let name = entry.name();

            // Match against entry name (folder or file)
            if let Some(score) = matcher.fuzzy_match(&name, query) {
                results.push(SearchResult {
                    path: path.to_path_buf(),
                    name,
                    score,
                });
            }
        }

        // Sort by score (highest first)
        results.sort_by(|a, b| b.score.cmp(&a.score).then(a.name.cmp(&b.name)));

        const MAX_RESULTS: usize = 50;
        if results.len() > MAX_RESULTS {
            results.truncate(MAX_RESULTS);
        }

        self.search_results = results;
        // Auto-select the most relevant result (first one with highest score)
        self.search_result_selected = 0;
    }

    fn execute_command(&mut self) -> Result<()> {
        let parts: Vec<&str> = self.command_input.split_whitespace().collect();

        if parts.is_empty() {
            return Ok(());
        }

        match parts[0] {
            "quit" | "q" => {
                self.should_quit = true;
            }
            "save" => {
                self.config.save()?;
                self.set_status(String::from("Configuration saved"));
            }
            "save-queue" | "savequeue" => {
                let name = if parts.len() > 1 {
                    Some(parts[1..].join(" "))
                } else {
                    None
                };
                self.save_queue_as_playlist(name.as_deref())?;
            }
            "vol" | "volume" => {
                if parts.len() > 1 {
                    if let Ok(vol) = parts[1].parse::<f32>() {
                        self.config.volume = (vol / 100.0).clamp(0.0, 1.0);
                        self.player.set_volume(self.config.volume);
                        self.set_status(format!("Volume: {:.0}%", self.config.volume * 100.0));
                    }
                }
            }
            _ => {
                self.set_status(format!("Unknown command: {}", parts[0]));
            }
        }

        self.command_input.clear();
        Ok(())
    }

    fn clear_search_results(&mut self) {
        self.search_results.clear();
        self.search_result_selected = 0;
    }

    fn save_queue_as_playlist(&mut self, name: Option<&str>) -> Result<()> {
        if self.queue.is_empty() {
            self.set_status(String::from("Queue is empty - nothing to save"));
            return Ok(());
        }

        let playlist_name = name
            .map(|n| n.trim())
            .filter(|n| !n.is_empty())
            .map(|n| n.to_string())
            .unwrap_or_else(Self::default_queue_playlist_name);

        let path = self
            .playlist_manager
            .save_playlist(&playlist_name, self.queue.tracks())?;

        self.set_status(format!(
            "Queue saved as '{}' at {}",
            playlist_name,
            path.display()
        ));

        Ok(())
    }

    fn default_queue_playlist_name() -> String {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_secs();
        format!("queue-{timestamp}")
    }

    fn play_next(&mut self) {
        // Scrobble current track if it should be scrobbled
        self.scrobble_if_needed();

        if let Some(track) = self.queue.next() {
            let track_clone = track.clone();
            let display_path = track_clone.display().to_string();
            if let Err(e) = self.player.play(track_clone.clone()) {
                self.set_status(format!("Error playing: {}", e));
            } else {
                self.set_status(format!("Playing: {}", display_path));
                self.start_track(&track_clone);
                // Sync queue selection to current playing track
                if let Some(current_idx) = self.queue.current_index() {
                    self.queue_selected = current_idx;
                }
            }
        }
    }

    fn play_prev(&mut self) {
        // Scrobble current track if it should be scrobbled
        self.scrobble_if_needed();

        if let Some(track) = self.queue.prev() {
            let track_clone = track.clone();
            let display_path = track_clone.display().to_string();
            if let Err(e) = self.player.play(track_clone.clone()) {
                self.set_status(format!("Error playing: {}", e));
            } else {
                self.set_status(format!("Playing: {}", display_path));
                self.start_track(&track_clone);
                // Sync queue selection to current playing track
                if let Some(current_idx) = self.queue.current_index() {
                    self.queue_selected = current_idx;
                }
            }
        }
    }

    fn draw(&mut self, f: &mut Frame) {
        // Determine if we should show the progress bar
        let show_progress = !self.queue.is_empty() && self.player.current_track().is_some();

        let constraints = if show_progress {
            vec![
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
                Constraint::Length(1),
            ]
        } else {
            vec![
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ]
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(f.area());

        self.draw_tabs(f, chunks[0]);
        self.draw_content(f, chunks[1]);
        if matches!(self.input_mode, InputMode::Search) && !self.search_results.is_empty() {
            self.draw_search_overlay(f);
        }

        if show_progress {
            self.draw_status(f, chunks[2]);
            self.draw_progress_bar(f, chunks[3]);
        } else {
            self.draw_status(f, chunks[2]);
        }
    }

    fn draw_tabs(&self, f: &mut Frame, area: Rect) {
        let tabs = [Tab::Browser, Tab::NowPlaying, Tab::Playlists];

        let mut tab_text = String::new();
        for (i, tab) in tabs.iter().enumerate() {
            if i > 0 {
                tab_text.push_str(" | ");
            }
            tab_text.push_str(&format!(" {} ", tab.name()));
        }

        let tabs_widget =
            Paragraph::new(tab_text).block(Block::default().borders(Borders::ALL).title("Tabs"));

        f.render_widget(tabs_widget, area);
    }

    fn draw_content(&mut self, f: &mut Frame, area: Rect) {
        match self.current_tab {
            Tab::Browser => self.draw_browser(f, area),
            Tab::NowPlaying => self.draw_now_playing(f, area),
            Tab::Playlists => self.draw_playlists(f, area),
        }
    }

    fn draw_browser(&mut self, f: &mut Frame, area: Rect) {
        let entries = self.browser.entries();
        let items: Vec<ListItem> = entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let style = if i == self.browser.selected() {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let prefix = if entry.is_dir() { "📁 " } else { "🎵 " };
                let content = format!("{}{}", prefix, entry.name());
                ListItem::new(content).style(style)
            })
            .collect();

        let title = format!("Browser: {}", self.browser.current_dir().display());
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        self.browser_state.select(Some(self.browser.selected()));
        f.render_stateful_widget(list, area, &mut self.browser_state);
    }

    fn draw_now_playing(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        self.draw_queue(f, chunks[0]);
        self.draw_player(f, chunks[1]);
    }

    fn draw_queue(&mut self, f: &mut Frame, area: Rect) {
        let tracks = self.queue.tracks();
        let current_index = self.queue.current_index();

        let items: Vec<ListItem> = tracks
            .iter()
            .enumerate()
            .map(|(i, track)| {
                let mut style = Style::default();

                if Some(i) == current_index {
                    style = style.fg(Color::Green);
                }

                if i == self.queue_selected {
                    style = style.add_modifier(Modifier::BOLD).fg(Color::Yellow);
                }

                let name = track
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                let prefix = if Some(i) == current_index {
                    "▶ "
                } else {
                    "  "
                };
                ListItem::new(format!("{}{}", prefix, name)).style(style)
            })
            .collect();

        let random_indicator = if self.queue.is_random() {
            " [RANDOM]"
        } else {
            ""
        };
        let title = format!("Queue ({} tracks){}", tracks.len(), random_indicator);
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        self.queue_state.select(Some(self.queue_selected));
        f.render_stateful_widget(list, area, &mut self.queue_state);
    }

    fn draw_player(&mut self, f: &mut Frame, area: Rect) {
        let metadata = self.player.current_metadata();

        // Split area with percentage-based layout: 55% for track metadata (left), 45% for album art (right)
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);

        // Render track metadata in left section (55% of width)
        let mut text = vec![];

        if let Some(ref meta) = metadata {
            // Title
            let title = meta.title.clone().unwrap_or_else(|| {
                self.player
                    .current_track()
                    .and_then(|t| {
                        t.file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| "Unknown".to_string())
            });
            text.push(Line::from(vec![
                Span::styled(
                    "Title: ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(title),
            ]));

            // Artist
            if let Some(ref artist) = meta.artist {
                text.push(Line::from(vec![
                    Span::styled("Artist: ", Style::default().fg(Color::Cyan)),
                    Span::raw(artist),
                ]));
            }

            // Album
            if let Some(ref album) = meta.album {
                text.push(Line::from(vec![
                    Span::styled("Album: ", Style::default().fg(Color::Cyan)),
                    Span::raw(album),
                ]));
            }

            // Album Artist (if different from artist)
            if let Some(ref album_artist) = meta.album_artist {
                if meta.artist.as_ref() != Some(album_artist) {
                    text.push(Line::from(vec![
                        Span::styled("Album Artist: ", Style::default().fg(Color::Cyan)),
                        Span::raw(album_artist),
                    ]));
                }
            }

            // Year
            if let Some(ref year) = meta.year {
                text.push(Line::from(vec![
                    Span::styled("Year: ", Style::default().fg(Color::Cyan)),
                    Span::raw(year),
                ]));
            }

            // Genre
            if let Some(ref genre) = meta.genre {
                text.push(Line::from(vec![
                    Span::styled("Genre: ", Style::default().fg(Color::Cyan)),
                    Span::raw(genre),
                ]));
            }

            // Track and Disc Number
            if let Some(ref track_num) = meta.track_number {
                let mut track_info = track_num.clone();
                if let Some(ref disc_num) = meta.disc_number {
                    track_info = format!("{} (Disc {})", track_num, disc_num);
                }
                text.push(Line::from(vec![
                    Span::styled("Track: ", Style::default().fg(Color::Cyan)),
                    Span::raw(track_info),
                ]));
            }

            // Duration
            text.push(Line::from(vec![
                Span::styled("Duration: ", Style::default().fg(Color::Cyan)),
                Span::raw(meta.format_duration()),
            ]));

            text.push(Line::from(""));

            // File path
            if let Some(track) = self.player.current_track() {
                text.push(Line::from(vec![
                    Span::styled("Path: ", Style::default().fg(Color::Gray)),
                    Span::raw(track.display().to_string()),
                ]));
            }
        } else if let Some(track) = self.player.current_track() {
            // No metadata available, show basic info
            let track_name = track
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown")
                .to_string();

            text.push(Line::from(vec![
                Span::styled("Now Playing: ", Style::default().fg(Color::Cyan)),
                Span::raw(track_name),
            ]));
            text.push(Line::from(""));
            text.push(Line::from(vec![
                Span::styled("Path: ", Style::default().fg(Color::Gray)),
                Span::raw(track.display().to_string()),
            ]));
        } else {
            text.push(Line::from("No track playing"));
        }

        text.push(Line::from(""));

        // Playback status
        let status = if self.player.is_playing() {
            "▶ Playing"
        } else if self.player.is_paused() {
            "⏸ Paused"
        } else {
            "⏹ Stopped"
        };

        text.push(Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::Cyan)),
            Span::raw(status),
        ]));

        text.push(Line::from(vec![
            Span::styled("Volume: ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("{:.0}%", self.config.volume * 100.0)),
        ]));

        let paragraph =
            Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Player"));

        f.render_widget(paragraph, chunks[0]);

        // Render album art in right section (45% of width)
        let current_track = self.player.current_track();

        // Check if we need to reload album art (track changed)
        let need_reload = {
            let last_track = self.last_album_art_track.lock().unwrap();
            *last_track != current_track
        };

        if need_reload {
            // Track changed, reload album art
            if let Some(ref meta) = metadata {
                if let Some(ref cover_data) = meta.cover_art {
                    // Try to load and display the album art
                    match image::load_from_memory(cover_data) {
                        Ok(img) => {
                            let mut picker = self.image_picker.lock().unwrap();
                            let mut album_art = self.album_art.lock().unwrap();

                            // Create or update the album art protocol
                            *album_art = Some(picker.new_resize_protocol(img));
                        }
                        Err(_) => {
                            // Failed to decode album art
                            *self.album_art.lock().unwrap() = None;
                        }
                    }
                } else {
                    // No album art available
                    *self.album_art.lock().unwrap() = None;
                }
            } else {
                // No metadata available
                *self.album_art.lock().unwrap() = None;
            }

            // Update last loaded track
            *self.last_album_art_track.lock().unwrap() = current_track.clone();
        }

        // Render the album art (whether cached or just loaded)
        let mut album_art = self.album_art.lock().unwrap();
        if let Some(ref mut protocol) = *album_art {
            // Create a block with borders for the album art
            let block = Block::default().borders(Borders::ALL).title("Album Art");
            let inner_area = block.inner(chunks[1]);
            f.render_widget(block, chunks[1]);

            let image_widget = ratatui_image::StatefulImage::new(None);
            f.render_stateful_widget(image_widget, inner_area, protocol);
        } else {
            // Show appropriate placeholder
            let (text, color) =
                if metadata.is_some() && metadata.as_ref().unwrap().cover_art.is_some() {
                    ("Invalid Album Art", Color::Red)
                } else {
                    ("No Album Art", Color::DarkGray)
                };

            let placeholder = Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL).title("Album Art"))
                .style(Style::default().fg(color));
            f.render_widget(placeholder, chunks[1]);
        }
    }

    fn draw_playlists(&mut self, f: &mut Frame, area: Rect) {
        let playlists = self.playlist_manager.playlists();

        let items: Vec<ListItem> = playlists
            .iter()
            .enumerate()
            .map(|(i, playlist)| {
                let style = if i == self.playlist_selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let content = format!("📋 {} ({} tracks)", playlist.name, playlist.tracks.len());
                ListItem::new(content).style(style)
            })
            .collect();

        let title = format!("Playlists ({})", playlists.len());
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        self.playlist_state.select(Some(self.playlist_selected));
        f.render_stateful_widget(list, area, &mut self.playlist_state);
    }

    fn draw_search_overlay(&mut self, f: &mut Frame) {
        let area = centered_rect(60, 50, f.area());

        // Clear the background to ensure the overlay is visible
        f.render_widget(Clear, area);

        let items: Vec<ListItem> = self
            .search_results
            .iter()
            .enumerate()
            .map(|(i, result)| {
                let mut style = Style::default();
                if i == self.search_result_selected {
                    style = style.fg(Color::Yellow).add_modifier(Modifier::BOLD);
                }

                // Show icon for folders vs files
                let prefix = if result.path.is_dir() {
                    "📁 "
                } else {
                    "🎵 "
                };
                let content = format!("{}{}", prefix, result.name);
                ListItem::new(content).style(style)
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Search Results"),
        );

        self.search_state.select(Some(self.search_result_selected));
        f.render_stateful_widget(list, area, &mut self.search_state);
    }

    fn draw_progress_bar(&self, f: &mut Frame, area: Rect) {
        // Get playback status icon
        let status_icon = if self.player.is_playing() {
            "▶"
        } else if self.player.is_paused() {
            "⏸"
        } else {
            "⏹"
        };

        // Get position and progress in a single call to minimize mutex locks
        let (position, progress) = self.player.get_position_and_progress();
        let progress = progress.unwrap_or(0.0);

        let position_secs = position.as_secs();
        let position_str = format!("{}:{:02}", position_secs / 60, position_secs % 60);

        let duration_str = if let Some(metadata) = self.player.current_metadata() {
            metadata.format_duration()
        } else {
            "?:??".to_string()
        };

        // Build the time label part
        let time_label = format!("{} {} / {} ", status_icon, position_str, duration_str);
        let time_label_len = time_label.len();

        // Calculate how many characters are available for the progress bar
        let available_width = (area.width as usize).saturating_sub(time_label_len);
        let filled_width = ((available_width as f64 * progress) as usize).min(available_width);
        let empty_width = available_width.saturating_sub(filled_width);

        let progress_bar = format!(
            "{}{}{}",
            time_label,
            "─".repeat(filled_width),
            "─".repeat(empty_width)
        );

        let paragraph = Paragraph::new(progress_bar).style(Style::default().fg(Color::Green));

        f.render_widget(paragraph, area);
    }

    fn draw_status(&self, f: &mut Frame, area: Rect) {
        let status_text = match self.input_mode {
            InputMode::Normal => {
                if self.status_message.is_empty() {
                    self.current_tab.help_text().to_string()
                } else {
                    self.status_message.clone()
                }
            }
            InputMode::Search => {
                if self.search_results.is_empty() {
                    format!("Search: {}", self.search_query)
                } else {
                    format!(
                        "Search: {} ({}/{} results, ↑/↓ navigate, Enter select, Esc cancel)",
                        self.search_query,
                        self.search_result_selected + 1,
                        self.search_results.len()
                    )
                }
            }
            InputMode::Command => format!(":{}", self.command_input),
        };

        let paragraph = Paragraph::new(status_text)
            .block(Block::default().borders(Borders::ALL).title("Status"));

        f.render_widget(paragraph, area);
    }

    fn start_track(&mut self, track: &Path) {
        *self.track_play_time.lock().unwrap() = Some(SystemTime::now());

        // Update now playing on Last.fm if enabled
        if self.lastfm_scrobbler.is_enabled() {
            if let Some(metadata) = self.player.current_metadata() {
                if let Err(e) = self.lastfm_scrobbler.now_playing(track, &metadata) {
                    eprintln!("Failed to update now playing on Last.fm: {}", e);
                }
            }
        }
    }

    fn scrobble_if_needed(&mut self) {
        if !self.lastfm_scrobbler.is_enabled() {
            return;
        }

        let track_start = self.track_play_time.lock().unwrap().take();
        if let Some(start_time) = track_start {
            if let Some(track) = self.player.current_track() {
                if let Some(metadata) = self.player.current_metadata() {
                    // According to Last.fm scrobbling rules:
                    // - Track must have been played for at least half its duration, or 4 minutes
                    let elapsed = SystemTime::now()
                        .duration_since(start_time)
                        .unwrap_or(Duration::from_secs(0))
                        .as_secs();

                    let should_scrobble = if let Some(duration) = metadata.duration_secs {
                        // Track must be played for at least half its duration or 4 minutes (whichever is lower)
                        elapsed >= (duration / 2).min(240)
                    } else {
                        // If we don't know the duration, scrobble after 4 minutes
                        elapsed >= 240
                    };

                    if should_scrobble {
                        if let Err(e) = self.lastfm_scrobbler.scrobble(&track, &metadata) {
                            eprintln!("Failed to scrobble track to Last.fm: {}", e);
                        }
                    }
                }
            }
        }

        self.lastfm_scrobbler.clear_current_track();
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
