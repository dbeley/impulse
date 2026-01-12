use anyhow::{Context, Result};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RepeatMode {
    #[default]
    Off,
    Queue,
    Track,
}

impl RepeatMode {
    pub fn next(&self) -> Self {
        match self {
            RepeatMode::Off => RepeatMode::Queue,
            RepeatMode::Queue => RepeatMode::Track,
            RepeatMode::Track => RepeatMode::Off,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            RepeatMode::Off => "Off",
            RepeatMode::Queue => "Queue",
            RepeatMode::Track => "Track",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Queue {
    tracks: Vec<PathBuf>,
    current_index: Option<usize>,
    random_mode: bool,
    #[serde(default)]
    repeat_mode: RepeatMode,
    #[serde(skip)]
    played_indices: Vec<usize>,
}

impl Queue {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            current_index: None,
            random_mode: false,
            repeat_mode: RepeatMode::Off,
            played_indices: Vec::new(),
        }
    }

    pub fn add(&mut self, track: PathBuf) {
        self.tracks.push(track);
        if self.current_index.is_none() && !self.tracks.is_empty() {
            self.current_index = Some(0);
        }
    }

    pub fn add_multiple(&mut self, tracks: Vec<PathBuf>) {
        for track in tracks {
            self.tracks.push(track);
        }
        if self.current_index.is_none() && !self.tracks.is_empty() {
            self.current_index = Some(0);
        }
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.tracks.len() {
            self.tracks.remove(index);

            // Update current_index if needed
            if let Some(current) = self.current_index {
                if current == index {
                    self.current_index = if self.tracks.is_empty() {
                        None
                    } else if current >= self.tracks.len() {
                        Some(self.tracks.len() - 1)
                    } else {
                        Some(current)
                    };
                } else if current > index {
                    self.current_index = Some(current - 1);
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.tracks.clear();
        self.current_index = None;
    }

    pub fn current(&self) -> Option<&PathBuf> {
        self.current_index.and_then(|idx| self.tracks.get(idx))
    }

    pub fn next(&mut self) -> Option<&PathBuf> {
        if self.tracks.is_empty() {
            return None;
        }

        if self.random_mode {
            // Mark current track as played
            if let Some(current) = self.current_index {
                if !self.played_indices.contains(&current) {
                    self.played_indices.push(current);
                }
            }

            // If all tracks have been played, reset
            if self.played_indices.len() >= self.tracks.len() {
                self.played_indices.clear();
            }

            // Find unplayed tracks
            let unplayed: Vec<usize> = (0..self.tracks.len())
                .filter(|i| !self.played_indices.contains(i))
                .collect();

            if !unplayed.is_empty() {
                let mut rng = rand::thread_rng();
                if let Some(&next_idx) = unplayed.choose(&mut rng) {
                    self.current_index = Some(next_idx);
                    return self.current();
                }
            }
            None
        } else {
            // Sequential mode
            if let Some(current) = self.current_index {
                if current + 1 < self.tracks.len() {
                    self.current_index = Some(current + 1);
                    return self.current();
                }
            }
            None
        }
    }

    pub fn prev(&mut self) -> Option<&PathBuf> {
        if let Some(current) = self.current_index {
            if current > 0 {
                self.current_index = Some(current - 1);
                return self.current();
            }
        }
        None
    }

    pub fn jump_to(&mut self, index: usize) -> Option<&PathBuf> {
        if index < self.tracks.len() {
            self.current_index = Some(index);
            self.current()
        } else {
            None
        }
    }

    pub fn tracks(&self) -> &[PathBuf] {
        &self.tracks
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    pub fn toggle_random(&mut self) {
        self.random_mode = !self.random_mode;
        if self.random_mode {
            self.played_indices.clear();
            // Disable track repeat when enabling random mode
            if self.repeat_mode == RepeatMode::Track {
                self.repeat_mode = RepeatMode::Off;
            }
        }
    }

    pub fn is_random(&self) -> bool {
        self.random_mode
    }

    pub fn cycle_repeat_mode(&mut self) {
        self.repeat_mode = self.repeat_mode.next();
        // Disable random mode when enabling track repeat
        if self.repeat_mode == RepeatMode::Track && self.random_mode {
            self.random_mode = false;
            self.played_indices.clear();
        }
    }

    pub fn repeat_mode(&self) -> RepeatMode {
        self.repeat_mode
    }

    pub fn restart(&mut self) -> Option<&PathBuf> {
        if !self.tracks.is_empty() {
            self.current_index = Some(0);
            if self.random_mode {
                self.played_indices.clear();
            }
            self.current()
        } else {
            None
        }
    }

    pub fn move_up(&mut self, index: usize) {
        if index == 0 || index >= self.tracks.len() {
            return;
        }

        self.tracks.swap(index, index - 1);
        if let Some(current) = self.current_index {
            if current == index {
                self.current_index = Some(index - 1);
            } else if current + 1 == index {
                self.current_index = Some(index);
            }
        }
    }

    pub fn move_down(&mut self, index: usize) {
        if index + 1 >= self.tracks.len() {
            return;
        }

        self.tracks.swap(index, index + 1);
        if let Some(current) = self.current_index {
            if current == index {
                self.current_index = Some(index + 1);
            } else if current == index + 1 {
                self.current_index = Some(index);
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let queue_path = Self::queue_path();

        if let Some(parent) = queue_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create queue directory {}", parent.display())
            })?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(&queue_path, content)
            .with_context(|| format!("Failed to write queue file at {}", queue_path.display()))?;
        Ok(())
    }

    pub fn load() -> Result<Self> {
        let queue_path = Self::queue_path();

        if queue_path.exists() {
            let content = fs::read_to_string(&queue_path)?;
            let queue: Queue = serde_json::from_str(&content)?;
            Ok(queue)
        } else {
            Ok(Queue::new())
        }
    }

    fn queue_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("impulse")
            .join("queue.json")
    }
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_queue_is_empty() {
        let queue = Queue::new();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
        assert!(queue.current().is_none());
        assert!(queue.current_index().is_none());
    }

    #[test]
    fn test_add_single_track() {
        let mut queue = Queue::new();
        let track = PathBuf::from("/music/track1.mp3");
        queue.add(track.clone());

        assert!(!queue.is_empty());
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.current(), Some(&track));
        assert_eq!(queue.current_index(), Some(0));
    }

    #[test]
    fn test_add_multiple_tracks() {
        let mut queue = Queue::new();
        let tracks = vec![
            PathBuf::from("/music/track1.mp3"),
            PathBuf::from("/music/track2.mp3"),
            PathBuf::from("/music/track3.mp3"),
        ];
        queue.add_multiple(tracks.clone());

        assert_eq!(queue.len(), 3);
        assert_eq!(queue.current(), Some(&tracks[0]));
        assert_eq!(queue.tracks(), &tracks[..]);
    }

    #[test]
    fn test_next_track() {
        let mut queue = Queue::new();
        let tracks = vec![
            PathBuf::from("/music/track1.mp3"),
            PathBuf::from("/music/track2.mp3"),
            PathBuf::from("/music/track3.mp3"),
        ];
        queue.add_multiple(tracks.clone());

        assert_eq!(queue.current(), Some(&tracks[0]));
        assert_eq!(queue.next(), Some(&tracks[1]));
        assert_eq!(queue.current(), Some(&tracks[1]));
        assert_eq!(queue.next(), Some(&tracks[2]));
        assert_eq!(queue.current(), Some(&tracks[2]));
        assert_eq!(queue.next(), None);
        assert_eq!(queue.current(), Some(&tracks[2]));
    }

    #[test]
    fn test_prev_track() {
        let mut queue = Queue::new();
        let tracks = vec![
            PathBuf::from("/music/track1.mp3"),
            PathBuf::from("/music/track2.mp3"),
            PathBuf::from("/music/track3.mp3"),
        ];
        queue.add_multiple(tracks.clone());
        queue.jump_to(2);

        assert_eq!(queue.current(), Some(&tracks[2]));
        assert_eq!(queue.prev(), Some(&tracks[1]));
        assert_eq!(queue.current(), Some(&tracks[1]));
        assert_eq!(queue.prev(), Some(&tracks[0]));
        assert_eq!(queue.current(), Some(&tracks[0]));
        assert_eq!(queue.prev(), None);
        assert_eq!(queue.current(), Some(&tracks[0]));
    }

    #[test]
    fn test_jump_to() {
        let mut queue = Queue::new();
        let tracks = vec![
            PathBuf::from("/music/track1.mp3"),
            PathBuf::from("/music/track2.mp3"),
            PathBuf::from("/music/track3.mp3"),
        ];
        queue.add_multiple(tracks.clone());

        assert_eq!(queue.jump_to(1), Some(&tracks[1]));
        assert_eq!(queue.current_index(), Some(1));
        assert_eq!(queue.jump_to(0), Some(&tracks[0]));
        assert_eq!(queue.current_index(), Some(0));
        assert_eq!(queue.jump_to(2), Some(&tracks[2]));
        assert_eq!(queue.current_index(), Some(2));
        assert_eq!(queue.jump_to(10), None);
        assert_eq!(queue.current_index(), Some(2));
    }

    #[test]
    fn test_remove_track() {
        let mut queue = Queue::new();
        let tracks = vec![
            PathBuf::from("/music/track1.mp3"),
            PathBuf::from("/music/track2.mp3"),
            PathBuf::from("/music/track3.mp3"),
        ];
        queue.add_multiple(tracks.clone());

        queue.remove(1);
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.tracks()[0], tracks[0]);
        assert_eq!(queue.tracks()[1], tracks[2]);
    }

    #[test]
    fn test_remove_current_track() {
        let mut queue = Queue::new();
        let tracks = vec![
            PathBuf::from("/music/track1.mp3"),
            PathBuf::from("/music/track2.mp3"),
            PathBuf::from("/music/track3.mp3"),
        ];
        queue.add_multiple(tracks.clone());
        queue.jump_to(1);

        queue.remove(1);
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.current_index(), Some(1));
        assert_eq!(queue.current(), Some(&tracks[2]));
    }

    #[test]
    fn test_remove_last_track_when_current() {
        let mut queue = Queue::new();
        let tracks = vec![
            PathBuf::from("/music/track1.mp3"),
            PathBuf::from("/music/track2.mp3"),
        ];
        queue.add_multiple(tracks.clone());
        queue.jump_to(1);

        queue.remove(1);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.current_index(), Some(0));
        assert_eq!(queue.current(), Some(&tracks[0]));
    }

    #[test]
    fn test_clear() {
        let mut queue = Queue::new();
        let tracks = vec![
            PathBuf::from("/music/track1.mp3"),
            PathBuf::from("/music/track2.mp3"),
        ];
        queue.add_multiple(tracks);

        queue.clear();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
        assert!(queue.current().is_none());
        assert!(queue.current_index().is_none());
    }

    #[test]
    fn test_move_up() {
        let mut queue = Queue::new();
        let tracks = vec![
            PathBuf::from("/music/track1.mp3"),
            PathBuf::from("/music/track2.mp3"),
            PathBuf::from("/music/track3.mp3"),
        ];
        queue.add_multiple(tracks.clone());

        queue.move_up(1);
        assert_eq!(queue.tracks()[0], tracks[1]);
        assert_eq!(queue.tracks()[1], tracks[0]);
        assert_eq!(queue.tracks()[2], tracks[2]);
    }

    #[test]
    fn test_move_up_first_item_does_nothing() {
        let mut queue = Queue::new();
        let tracks = vec![
            PathBuf::from("/music/track1.mp3"),
            PathBuf::from("/music/track2.mp3"),
        ];
        queue.add_multiple(tracks.clone());

        queue.move_up(0);
        assert_eq!(queue.tracks()[0], tracks[0]);
        assert_eq!(queue.tracks()[1], tracks[1]);
    }

    #[test]
    fn test_move_down() {
        let mut queue = Queue::new();
        let tracks = vec![
            PathBuf::from("/music/track1.mp3"),
            PathBuf::from("/music/track2.mp3"),
            PathBuf::from("/music/track3.mp3"),
        ];
        queue.add_multiple(tracks.clone());

        queue.move_down(0);
        assert_eq!(queue.tracks()[0], tracks[1]);
        assert_eq!(queue.tracks()[1], tracks[0]);
        assert_eq!(queue.tracks()[2], tracks[2]);
    }

    #[test]
    fn test_move_down_last_item_does_nothing() {
        let mut queue = Queue::new();
        let tracks = vec![
            PathBuf::from("/music/track1.mp3"),
            PathBuf::from("/music/track2.mp3"),
        ];
        queue.add_multiple(tracks.clone());

        queue.move_down(1);
        assert_eq!(queue.tracks()[0], tracks[0]);
        assert_eq!(queue.tracks()[1], tracks[1]);
    }

    #[test]
    fn test_move_up_updates_current_index() {
        let mut queue = Queue::new();
        let tracks = vec![
            PathBuf::from("/music/track1.mp3"),
            PathBuf::from("/music/track2.mp3"),
            PathBuf::from("/music/track3.mp3"),
        ];
        queue.add_multiple(tracks);
        queue.jump_to(1);

        queue.move_up(1);
        assert_eq!(queue.current_index(), Some(0));
    }

    #[test]
    fn test_move_down_updates_current_index() {
        let mut queue = Queue::new();
        let tracks = vec![
            PathBuf::from("/music/track1.mp3"),
            PathBuf::from("/music/track2.mp3"),
            PathBuf::from("/music/track3.mp3"),
        ];
        queue.add_multiple(tracks);
        queue.jump_to(1);

        queue.move_down(1);
        assert_eq!(queue.current_index(), Some(2));
    }

    #[test]
    fn test_toggle_random() {
        let mut queue = Queue::new();
        assert!(!queue.is_random());

        queue.toggle_random();
        assert!(queue.is_random());

        queue.toggle_random();
        assert!(!queue.is_random());
    }

    #[test]
    fn test_random_mode_plays_all_tracks() {
        let mut queue = Queue::new();
        let tracks = vec![
            PathBuf::from("/music/track1.mp3"),
            PathBuf::from("/music/track2.mp3"),
            PathBuf::from("/music/track3.mp3"),
            PathBuf::from("/music/track4.mp3"),
            PathBuf::from("/music/track5.mp3"),
        ];
        queue.add_multiple(tracks.clone());
        queue.toggle_random();

        let mut played = std::collections::HashSet::new();

        // Play through all tracks
        for _ in 0..tracks.len() {
            if let Some(track) = queue.current() {
                played.insert(track.clone());
            }
            queue.next();
        }

        // Should have played all unique tracks
        assert_eq!(played.len(), tracks.len());
    }

    #[test]
    fn test_random_mode_resets_after_all_played() {
        let mut queue = Queue::new();
        let tracks = vec![
            PathBuf::from("/music/track1.mp3"),
            PathBuf::from("/music/track2.mp3"),
            PathBuf::from("/music/track3.mp3"),
        ];
        queue.add_multiple(tracks.clone());
        queue.toggle_random();

        // Play through all tracks
        for _ in 0..tracks.len() {
            queue.next();
        }

        // After playing all, should be able to play again
        let next_track = queue.current();
        assert!(next_track.is_some());
    }

    #[test]
    fn test_sequential_mode_after_random() {
        let mut queue = Queue::new();
        let tracks = vec![
            PathBuf::from("/music/track1.mp3"),
            PathBuf::from("/music/track2.mp3"),
            PathBuf::from("/music/track3.mp3"),
        ];
        queue.add_multiple(tracks.clone());

        // Enable random mode
        queue.toggle_random();
        queue.next();

        // Disable random mode
        queue.toggle_random();

        // Should work sequentially now
        queue.jump_to(0);
        assert_eq!(queue.next(), Some(&tracks[1]));
        assert_eq!(queue.next(), Some(&tracks[2]));
    }
}
