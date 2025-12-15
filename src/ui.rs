use crate::browser::{is_audio_file, Browser};
use crate::config::Config;
use crate::lastfm::LastfmScrobbler;
use crate::player::Player;
use crate::playlist::PlaylistManager;
use crate::queue::Queue;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tab {
    Browser,
    Queue,
    Player,
    Playlists,
}

impl Tab {
    fn next(&self) -> Self {
        match self {
            Tab::Browser => Tab::Queue,
            Tab::Queue => Tab::Player,
            Tab::Player => Tab::Playlists,
            Tab::Playlists => Tab::Browser,
        }
    }

    fn prev(&self) -> Self {
        match self {
            Tab::Browser => Tab::Playlists,
            Tab::Queue => Tab::Browser,
            Tab::Player => Tab::Queue,
            Tab::Playlists => Tab::Player,
        }
    }

    fn name(&self) -> &str {
        match self {
            Tab::Browser => "1. Browser",
            Tab::Queue => "2. Queue",
            Tab::Player => "3. Player",
            Tab::Playlists => "4. Playlists",
        }
    }
}

pub enum InputMode {
    Normal,
    Search,
    Command,
    SearchResults,
}

struct SearchResult {
    path: PathBuf,
    folder: PathBuf,
    name: String,
    score: i64,
    depth: usize,
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
    image_picker: Arc<Mutex<Picker>>,
    album_art: Arc<Mutex<Option<Box<dyn StatefulProtocol>>>>,
    lastfm_scrobbler: LastfmScrobbler,
    track_play_time: Arc<Mutex<Option<SystemTime>>>,
}

impl App {
    pub fn new(config: Config) -> Result<Self> {
        let player = Player::new()?;
        player.set_volume(config.volume);

        let browser = Browser::new(config.music_dir.clone());
        let queue = Queue::new();
        let playlist_manager = PlaylistManager::new(config.playlist_dir.clone());

        // Initialize image picker for album art display
        let mut picker = Picker::from_termios().unwrap_or_else(|_| Picker::new((8, 12)));
        picker.guess_protocol();

        // Initialize Last.fm scrobbler
        let lastfm_scrobbler = LastfmScrobbler::new(config.lastfm.as_ref())?;

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
            status_message: String::from("Welcome to Impulse! Press '?' for help"),
            image_picker: Arc::new(Mutex::new(picker)),
            album_art: Arc::new(Mutex::new(None)),
            lastfm_scrobbler,
            track_play_time: Arc::new(Mutex::new(None)),
        })
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            terminal.draw(|f| self.draw(f))?;

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key)?;
                }
            }

            // Check if current track finished
            if self.player.is_finished() && !self.queue.is_empty() {
                // Scrobble the finished track if enough time has passed
                self.scrobble_if_needed();
                self.play_next();
            } else if self.player.is_finished() {
                // Track finished but queue is empty, scrobble it
                self.scrobble_if_needed();
            }

            if self.should_quit {
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
            InputMode::SearchResults => self.handle_search_results_keys(key)?,
        }
        Ok(())
    }

    fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Char('?') => {
                self.status_message = String::from(
                    "Keys: j/k/↑/↓=nav, l/→/Enter=select, h/←=back, Space=play/pause, n=next, p=prev, a=add, A=add-all, Tab/1-4=switch-tab, /=search, q=quit"
                );
            }
            KeyCode::Char('1') => {
                self.current_tab = Tab::Browser;
            }
            KeyCode::Char('2') => {
                self.current_tab = Tab::Queue;
            }
            KeyCode::Char('3') => {
                self.current_tab = Tab::Player;
            }
            KeyCode::Char('4') => {
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
            KeyCode::Char(' ') => {
                if self.player.is_playing() {
                    self.player.pause();
                    self.status_message = String::from("Paused");
                } else if self.player.is_paused() {
                    self.player.resume();
                    self.status_message = String::from("Resumed");
                } else if let Some(track) = self.queue.current() {
                    let track_clone = track.clone();
                    if let Err(e) = self.player.play(track_clone.clone()) {
                        self.status_message = format!("Error playing: {}", e);
                    } else {
                        self.status_message = format!("Playing: {}", track_clone.display());
                        self.start_track(&track_clone);
                    }
                }
            }
            KeyCode::Char('n') | KeyCode::Char('>') => {
                self.play_next();
            }
            KeyCode::Char('P') | KeyCode::Char('<') => {
                self.play_prev();
            }
            KeyCode::Char('s') => {
                self.scrobble_if_needed();
                self.player.stop();
                self.status_message = String::from("Stopped");
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
            Tab::Queue => self.handle_queue_keys(key)?,
            Tab::Player => self.handle_player_keys(key)?,
            Tab::Playlists => self.handle_playlist_keys(key)?,
        }
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
            KeyCode::Char('l') | KeyCode::Enter | KeyCode::Right => {
                if let Some(track) = self.browser.enter_selected() {
                    self.queue.add(track.clone());
                    self.status_message = format!("Added to queue: {}", track.display());

                    if self.queue.len() == 1 && !self.player.is_playing() {
                        if let Err(e) = self.player.play(track.clone()) {
                            self.status_message = format!("Error playing: {}", e);
                        } else {
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
                        self.status_message = format!("Added to queue: {}", entry.name());
                    }
                }
            }
            KeyCode::Char('A') => {
                let files = self.browser.get_all_audio_files();
                let count = files.len();
                self.queue.add_multiple(files);
                self.status_message = format!("Added {} tracks to queue", count);
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
            KeyCode::Enter => {
                // Scrobble current track if needed before jumping
                self.scrobble_if_needed();

                if let Some(track) = self.queue.jump_to(self.queue_selected) {
                    let track_clone = track.clone();
                    if let Err(e) = self.player.play(track_clone.clone()) {
                        self.status_message = format!("Error playing: {}", e);
                    } else {
                        self.status_message = format!("Playing: {}", track_clone.display());
                        self.start_track(&track_clone);
                    }
                }
            }
            KeyCode::Char('d') | KeyCode::Delete | KeyCode::Backspace => {
                if !self.queue.is_empty() {
                    self.queue.remove(self.queue_selected);
                    self.status_message = String::from("Removed from queue");
                    if self.queue_selected >= self.queue.len() && !self.queue.is_empty() {
                        self.queue_selected = self.queue.len() - 1;
                    }
                }
            }
            KeyCode::Char('K') => {
                if self.queue_selected > 0 {
                    self.queue.move_up(self.queue_selected);
                    self.queue_selected -= 1;
                    self.status_message = String::from("Moved track up");
                }
            }
            KeyCode::Char('J') => {
                if self.queue_selected + 1 < self.queue.len() {
                    self.queue.move_down(self.queue_selected);
                    self.queue_selected += 1;
                    self.status_message = String::from("Moved track down");
                }
            }
            KeyCode::Char('c') => {
                self.queue.clear();
                self.queue_selected = 0;
                self.status_message = String::from("Queue cleared");
            }
            KeyCode::Char('S') => {
                if self.queue.is_empty() {
                    self.status_message = String::from("Queue is empty - nothing to save");
                } else {
                    self.input_mode = InputMode::Command;
                    self.command_input = String::from("save-queue ");
                    self.status_message =
                        String::from("Enter name to save queue as playlist (default folder)");
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_player_keys(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.config.volume = (self.config.volume + 0.1).min(1.0);
                self.player.set_volume(self.config.volume);
                self.status_message = format!("Volume: {:.0}%", self.config.volume * 100.0);
            }
            KeyCode::Char('-') => {
                self.config.volume = (self.config.volume - 0.1).max(0.0);
                self.player.set_volume(self.config.volume);
                self.status_message = format!("Volume: {:.0}%", self.config.volume * 100.0);
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
            KeyCode::Char('l') | KeyCode::Enter => {
                if let Some(playlist) = self.playlist_manager.get_playlist(self.playlist_selected) {
                    self.queue.add_multiple(playlist.tracks.clone());
                    self.status_message = format!("Added playlist '{}' to queue", playlist.name);
                }
            }
            KeyCode::Char('r') => {
                self.playlist_manager.load_playlists();
                self.status_message = String::from("Playlists reloaded");
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_search_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Enter => {
                let results = self.collect_search_results();
                if results.is_empty() {
                    self.status_message = format!("No match found for: {}", self.search_query);
                    self.input_mode = InputMode::Normal;
                } else {
                    self.search_results = results;
                    self.search_result_selected = 0;
                    self.input_mode = InputMode::SearchResults;
                    self.status_message = format!(
                        "{} match(es) for '{}'",
                        self.search_results.len(),
                        self.search_query
                    );
                }
            }
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.search_query.clear();
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
            }
            KeyCode::Backspace => {
                self.search_query.pop();
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

    fn handle_search_results_keys(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.search_results.is_empty() {
                    self.search_result_selected =
                        (self.search_result_selected + 1).min(self.search_results.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.search_result_selected = self.search_result_selected.saturating_sub(1);
            }
            KeyCode::Enter => {
                if let Some(result) = self.search_results.get(self.search_result_selected) {
                    self.browser.navigate_to(result.folder.clone());
                    self.browser.select_entry_by_path(&result.path);
                    self.status_message = format!("Found track in: {}", result.folder.display());
                }
                self.input_mode = InputMode::Normal;
                self.clear_search_results();
            }
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.clear_search_results();
                self.status_message = String::from("Search canceled");
            }
            _ => {}
        }
        Ok(())
    }

    fn collect_search_results(&self) -> Vec<SearchResult> {
        let query = self.search_query.trim();
        if query.is_empty() {
            return Vec::new();
        }

        let matcher = SkimMatcherV2::default();
        let mut results = Vec::new();

        for entry in WalkDir::new(self.browser.current_dir())
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() || !is_audio_file(path) {
                continue;
            }

            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            if let Some(score) = matcher.fuzzy_match(&name, query) {
                let folder = path
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| self.browser.current_dir().to_path_buf());
                results.push(SearchResult {
                    path: path.to_path_buf(),
                    folder,
                    name,
                    score,
                    depth: entry.depth(),
                });
            }
        }

        results.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then(a.depth.cmp(&b.depth))
                .then(a.name.cmp(&b.name))
        });

        const MAX_RESULTS: usize = 30;
        if results.len() > MAX_RESULTS {
            results.truncate(MAX_RESULTS);
        }

        results
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
                self.status_message = String::from("Configuration saved");
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
                        self.status_message = format!("Volume: {:.0}%", self.config.volume * 100.0);
                    }
                }
            }
            _ => {
                self.status_message = format!("Unknown command: {}", parts[0]);
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
            self.status_message = String::from("Queue is empty - nothing to save");
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

        self.status_message = format!("Queue saved as '{}' at {}", playlist_name, path.display());

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
            if let Err(e) = self.player.play(track_clone.clone()) {
                self.status_message = format!("Error playing: {}", e);
            } else {
                self.status_message = format!("Playing: {}", track_clone.display());
                self.start_track(&track_clone);
            }
        }
    }

    fn play_prev(&mut self) {
        // Scrobble current track if it should be scrobbled
        self.scrobble_if_needed();

        if let Some(track) = self.queue.prev() {
            let track_clone = track.clone();
            if let Err(e) = self.player.play(track_clone.clone()) {
                self.status_message = format!("Error playing: {}", e);
            } else {
                self.status_message = format!("Playing: {}", track_clone.display());
                self.start_track(&track_clone);
            }
        }
    }

    fn draw(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(f.size());

        self.draw_tabs(f, chunks[0]);
        self.draw_content(f, chunks[1]);
        if matches!(self.input_mode, InputMode::SearchResults) {
            self.draw_search_overlay(f);
        }
        self.draw_status(f, chunks[2]);
    }

    fn draw_tabs(&self, f: &mut Frame, area: Rect) {
        let tabs = [Tab::Browser, Tab::Queue, Tab::Player, Tab::Playlists];

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
            Tab::Queue => self.draw_queue(f, area),
            Tab::Player => self.draw_player(f, area),
            Tab::Playlists => self.draw_playlists(f, area),
        }
    }

    fn draw_browser(&self, f: &mut Frame, area: Rect) {
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

        f.render_widget(list, area);
    }

    fn draw_queue(&self, f: &mut Frame, area: Rect) {
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

        let title = format!("Queue ({} tracks)", tracks.len());
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        f.render_widget(list, area);
    }

    fn draw_player(&mut self, f: &mut Frame, area: Rect) {
        let metadata = self.player.current_metadata();

        // Split area: left for album art, right for metadata
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(40), Constraint::Min(0)])
            .split(area);

        // Draw album art if available
        if let Some(ref meta) = metadata {
            if let Some(ref cover_data) = meta.cover_art {
                // Try to load and display the album art
                match image::load_from_memory(cover_data) {
                    Ok(img) => {
                        let mut picker = self.image_picker.lock().unwrap();
                        let mut album_art = self.album_art.lock().unwrap();

                        // Create or update the album art protocol
                        *album_art = Some(picker.new_resize_protocol(img));

                        if let Some(ref mut protocol) = *album_art {
                            let image_widget = ratatui_image::StatefulImage::new(None);
                            drop(picker); // Release lock before rendering
                            f.render_stateful_widget(image_widget, chunks[0], protocol);
                        }
                    }
                    Err(_) => {
                        // Failed to decode album art
                        let placeholder = Paragraph::new("Invalid Album Art")
                            .block(Block::default().borders(Borders::ALL))
                            .style(Style::default().fg(Color::Red));
                        f.render_widget(placeholder, chunks[0]);
                    }
                }
            } else {
                // No album art - show placeholder
                let placeholder = Paragraph::new("No Album Art")
                    .block(Block::default().borders(Borders::ALL))
                    .style(Style::default().fg(Color::DarkGray));
                f.render_widget(placeholder, chunks[0]);
            }
        } else {
            let placeholder = Paragraph::new("No Album Art")
                .block(Block::default().borders(Borders::ALL))
                .style(Style::default().fg(Color::DarkGray));
            f.render_widget(placeholder, chunks[0]);
        }

        // Draw metadata
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

        f.render_widget(paragraph, chunks[1]);
    }

    fn draw_playlists(&self, f: &mut Frame, area: Rect) {
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

        f.render_widget(list, area);
    }

    fn draw_search_overlay(&self, f: &mut Frame) {
        let area = centered_rect(60, 50, f.size());
        let items: Vec<ListItem> = self
            .search_results
            .iter()
            .enumerate()
            .map(|(i, result)| {
                let mut style = Style::default();
                if i == self.search_result_selected {
                    style = style.fg(Color::Yellow).add_modifier(Modifier::BOLD);
                }

                let content = format!("{} — {}", result.name, result.folder.display());
                ListItem::new(content).style(style)
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Search Results"),
        );

        f.render_widget(list, area);
    }

    fn draw_status(&self, f: &mut Frame, area: Rect) {
        let status_text = match self.input_mode {
            InputMode::Normal => self.status_message.clone(),
            InputMode::Search => format!("Search: {}", self.search_query),
            InputMode::Command => format!(":{}", self.command_input),
            InputMode::SearchResults => format!(
                "Search results: {}/{} (↑/↓ Navigate, Enter open, Esc cancel)",
                self.search_result_selected + 1,
                self.search_results.len()
            ),
        };

        let paragraph = Paragraph::new(status_text)
            .block(Block::default().borders(Borders::ALL).title("Status"));

        f.render_widget(paragraph, area);
    }

    fn start_track(&mut self, track: &PathBuf) {
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
