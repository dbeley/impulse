use crate::browser::Browser;
use crate::config::Config;
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
use std::time::Duration;

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
            Tab::Browser => "Browser",
            Tab::Queue => "Queue",
            Tab::Player => "Player",
            Tab::Playlists => "Playlists",
        }
    }
}

pub enum InputMode {
    Normal,
    Search,
    Command,
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
    queue_selected: usize,
    playlist_selected: usize,
    playlist_track_selected: usize,
    should_quit: bool,
    status_message: String,
}

impl App {
    pub fn new(config: Config) -> Result<Self> {
        let player = Player::new()?;
        player.set_volume(config.volume);
        
        let browser = Browser::new(config.music_dir.clone());
        let queue = Queue::new();
        let playlist_manager = PlaylistManager::new(config.playlist_dir.clone());

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
            queue_selected: 0,
            playlist_selected: 0,
            playlist_track_selected: 0,
            should_quit: false,
            status_message: String::from("Welcome to Impulse! Press '?' for help"),
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
                self.play_next();
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
                    "Keys: j/k=nav, l/Enter=select, h=back, Space=play/pause, n=next, p=prev, a=add, A=add-all, Tab=switch-tab, /=search, q=quit"
                );
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
                    self.player.play(track.clone())?;
                    self.status_message = format!("Playing: {}", track.display());
                }
            }
            KeyCode::Char('n') => {
                self.play_next();
            }
            KeyCode::Char('P') => {
                self.play_prev();
            }
            KeyCode::Char('s') => {
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
            KeyCode::Char('l') | KeyCode::Enter => {
                if let Some(track) = self.browser.enter_selected() {
                    self.queue.add(track.clone());
                    self.status_message = format!("Added to queue: {}", track.display());
                    
                    if self.queue.len() == 1 && !self.player.is_playing() {
                        self.player.play(track)?;
                    }
                }
            }
            KeyCode::Char('h') => {
                if let Some(parent) = self.browser.current_dir().parent() {
                    self.browser = Browser::new(parent.to_path_buf());
                }
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
                if let Some(track) = self.queue.jump_to(self.queue_selected) {
                    self.player.play(track.clone())?;
                    self.status_message = format!("Playing: {}", track.display());
                }
            }
            KeyCode::Char('d') => {
                if !self.queue.is_empty() {
                    self.queue.remove(self.queue_selected);
                    self.status_message = String::from("Removed from queue");
                    if self.queue_selected >= self.queue.len() && !self.queue.is_empty() {
                        self.queue_selected = self.queue.len() - 1;
                    }
                }
            }
            KeyCode::Char('c') => {
                self.queue.clear();
                self.queue_selected = 0;
                self.status_message = String::from("Queue cleared");
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
                self.input_mode = InputMode::Normal;
                self.perform_search();
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

    fn perform_search(&mut self) {
        if self.search_query.is_empty() {
            return;
        }

        let matcher = SkimMatcherV2::default();
        let entries: Vec<_> = self.browser.entries().to_vec();
        
        for (_i, entry) in entries.iter().enumerate() {
            if let Some(_score) = matcher.fuzzy_match(&entry.name(), &self.search_query) {
                // Found a match
                self.status_message = format!("Found: {}", entry.name());
                return;
            }
        }

        self.status_message = format!("No match found for: {}", self.search_query);
    }

    fn execute_command(&mut self) -> Result<()> {
        let parts: Vec<&str> = self.command_input.trim().split_whitespace().collect();
        
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

    fn play_next(&mut self) {
        if let Some(track) = self.queue.next() {
            if let Err(e) = self.player.play(track.clone()) {
                self.status_message = format!("Error playing: {}", e);
            } else {
                self.status_message = format!("Playing: {}", track.display());
            }
        }
    }

    fn play_prev(&mut self) {
        if let Some(track) = self.queue.prev() {
            if let Err(e) = self.player.play(track.clone()) {
                self.status_message = format!("Error playing: {}", e);
            } else {
                self.status_message = format!("Playing: {}", track.display());
            }
        }
    }

    fn draw(&self, f: &mut Frame) {
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
        self.draw_status(f, chunks[2]);
    }

    fn draw_tabs(&self, f: &mut Frame, area: Rect) {
        let tabs = vec![
            Tab::Browser,
            Tab::Queue,
            Tab::Player,
            Tab::Playlists,
        ];

        let mut tab_text = String::new();
        for (i, tab) in tabs.iter().enumerate() {
            if i > 0 {
                tab_text.push_str(" | ");
            }
            tab_text.push_str(&format!(" {} ", tab.name()));
        }

        let tabs_widget = Paragraph::new(tab_text)
            .block(Block::default().borders(Borders::ALL).title("Tabs"));

        f.render_widget(tabs_widget, area);
    }

    fn draw_content(&self, f: &mut Frame, area: Rect) {
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
                
                let prefix = if Some(i) == current_index { "▶ " } else { "  " };
                ListItem::new(format!("{}{}", prefix, name)).style(style)
            })
            .collect();

        let title = format!("Queue ({} tracks)", tracks.len());
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        f.render_widget(list, area);
    }

    fn draw_player(&self, f: &mut Frame, area: Rect) {
        let mut text = vec![];

        if let Some(track) = self.player.current_track() {
            let track_name = track
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown")
                .to_string();
            let track_path = track.display().to_string();
            
            text.push(Line::from(vec![
                Span::styled("Now Playing: ", Style::default().fg(Color::Cyan)),
                Span::raw(track_name),
            ]));
            text.push(Line::from(""));
            text.push(Line::from(vec![
                Span::styled("Path: ", Style::default().fg(Color::Gray)),
                Span::raw(track_path),
            ]));
        } else {
            text.push(Line::from("No track playing"));
        }

        text.push(Line::from(""));
        
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

        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Player"));

        f.render_widget(paragraph, area);
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

    fn draw_status(&self, f: &mut Frame, area: Rect) {
        let status_text = match self.input_mode {
            InputMode::Normal => self.status_message.clone(),
            InputMode::Search => format!("Search: {}", self.search_query),
            InputMode::Command => format!(":{}", self.command_input),
        };

        let paragraph = Paragraph::new(status_text)
            .block(Block::default().borders(Borders::ALL).title("Status"));

        f.render_widget(paragraph, area);
    }
}
