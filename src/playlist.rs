use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Playlist {
    pub name: String,
    #[allow(dead_code)]
    pub path: PathBuf,
    pub tracks: Vec<PathBuf>,
}

impl Playlist {
    #[allow(dead_code)]
    pub fn new(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            tracks: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn load(path: &Path) -> Result<Self> {
        let content =
            fs::read_to_string(path).context(format!("Failed to read playlist: {:?}", path))?;

        let name = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("Unnamed")
            .to_string();

        let mut tracks = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                tracks.push(PathBuf::from(line));
            }
        }

        Ok(Self {
            name,
            path: path.to_path_buf(),
            tracks,
        })
    }

    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        let mut content = String::new();
        content.push_str("#EXTM3U\n");

        for track in &self.tracks {
            if let Some(track_str) = track.to_str() {
                content.push_str(track_str);
                content.push('\n');
            }
        }

        fs::write(&self.path, content)
            .context(format!("Failed to save playlist: {:?}", self.path))?;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn add_track(&mut self, track: PathBuf) {
        self.tracks.push(track);
    }

    #[allow(dead_code)]
    pub fn remove_track(&mut self, index: usize) {
        if index < self.tracks.len() {
            self.tracks.remove(index);
        }
    }
}

pub struct PlaylistManager {
    playlist_dir: PathBuf,
    playlists: Vec<Playlist>,
}

impl PlaylistManager {
    pub fn new(playlist_dir: PathBuf) -> Self {
        let mut manager = Self {
            playlist_dir,
            playlists: Vec::new(),
        };
        manager.load_playlists();
        manager
    }

    pub fn load_playlists(&mut self) {
        self.playlists.clear();

        if !self.playlist_dir.exists() {
            return;
        }

        if let Ok(entries) = fs::read_dir(&self.playlist_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "m3u" || ext == "m3u8" {
                            if let Ok(playlist) = Playlist::load(&path) {
                                self.playlists.push(playlist);
                            }
                        }
                    }
                }
            }
        }

        self.playlists.sort_by(|a, b| a.name.cmp(&b.name));
    }

    #[allow(dead_code)]
    pub fn create_playlist(&mut self, name: String) -> Result<()> {
        let filename = format!("{}.m3u", name.replace(['/', '\\'], "_"));
        let path = self.playlist_dir.join(filename);

        let playlist = Playlist::new(name, path);
        playlist.save()?;
        self.playlists.push(playlist);

        Ok(())
    }

    #[allow(dead_code)]
    pub fn delete_playlist(&mut self, index: usize) -> Result<()> {
        if index < self.playlists.len() {
            let playlist = &self.playlists[index];
            fs::remove_file(&playlist.path)
                .context(format!("Failed to delete playlist: {:?}", playlist.path))?;
            self.playlists.remove(index);
        }
        Ok(())
    }

    pub fn save_playlist(&mut self, name: &str, tracks: &[PathBuf]) -> Result<PathBuf> {
        if tracks.is_empty() {
            return Err(anyhow!("Cannot save an empty playlist"));
        }

        if !self.playlist_dir.exists() {
            fs::create_dir_all(&self.playlist_dir).context(format!(
                "Failed to create playlist directory: {:?}",
                self.playlist_dir
            ))?;
        }

        let filename = playlist_filename(name);
        let path = self.playlist_dir.join(filename);

        let display_name = if name.trim().is_empty() {
            "playlist".to_string()
        } else {
            name.trim().to_string()
        };

        let playlist = Playlist {
            name: display_name,
            path: path.clone(),
            tracks: tracks.to_vec(),
        };

        playlist.save()?;
        self.load_playlists();
        Ok(path)
    }

    pub fn playlists(&self) -> &[Playlist] {
        &self.playlists
    }

    #[allow(dead_code)]
    pub fn playlists_mut(&mut self) -> &mut [Playlist] {
        &mut self.playlists
    }

    pub fn get_playlist(&self, index: usize) -> Option<&Playlist> {
        self.playlists.get(index)
    }

    #[allow(dead_code)]
    pub fn get_playlist_mut(&mut self, index: usize) -> Option<&mut Playlist> {
        self.playlists.get_mut(index)
    }
}

fn playlist_filename(name: &str) -> String {
    let trimmed = name.trim();
    let mut base = if trimmed.is_empty() {
        "playlist".to_string()
    } else {
        trimmed.replace(['/', '\\'], "_")
    };

    if !(base.ends_with(".m3u") || base.ends_with(".m3u8")) {
        base.push_str(".m3u");
    }

    base
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_playlist_new() {
        let playlist = Playlist::new("Test".to_string(), PathBuf::from("/test/playlist.m3u"));
        assert_eq!(playlist.name, "Test");
        assert_eq!(playlist.path, PathBuf::from("/test/playlist.m3u"));
        assert!(playlist.tracks.is_empty());
    }

    #[test]
    fn test_playlist_add_track() {
        let mut playlist = Playlist::new("Test".to_string(), PathBuf::from("/test/playlist.m3u"));
        playlist.add_track(PathBuf::from("/music/track1.mp3"));
        playlist.add_track(PathBuf::from("/music/track2.mp3"));

        assert_eq!(playlist.tracks.len(), 2);
        assert_eq!(playlist.tracks[0], PathBuf::from("/music/track1.mp3"));
        assert_eq!(playlist.tracks[1], PathBuf::from("/music/track2.mp3"));
    }

    #[test]
    fn test_playlist_remove_track() {
        let mut playlist = Playlist::new("Test".to_string(), PathBuf::from("/test/playlist.m3u"));
        playlist.add_track(PathBuf::from("/music/track1.mp3"));
        playlist.add_track(PathBuf::from("/music/track2.mp3"));
        playlist.add_track(PathBuf::from("/music/track3.mp3"));

        playlist.remove_track(1);
        assert_eq!(playlist.tracks.len(), 2);
        assert_eq!(playlist.tracks[0], PathBuf::from("/music/track1.mp3"));
        assert_eq!(playlist.tracks[1], PathBuf::from("/music/track3.mp3"));
    }

    #[test]
    fn test_playlist_remove_track_out_of_bounds() {
        let mut playlist = Playlist::new("Test".to_string(), PathBuf::from("/test/playlist.m3u"));
        playlist.add_track(PathBuf::from("/music/track1.mp3"));

        playlist.remove_track(5);
        assert_eq!(playlist.tracks.len(), 1);
    }

    #[test]
    fn test_playlist_filename_basic() {
        assert_eq!(playlist_filename("my playlist"), "my playlist.m3u");
        assert_eq!(playlist_filename("test"), "test.m3u");
    }

    #[test]
    fn test_playlist_filename_with_slashes() {
        assert_eq!(playlist_filename("my/playlist"), "my_playlist.m3u");
        assert_eq!(playlist_filename("my\\playlist"), "my_playlist.m3u");
    }

    #[test]
    fn test_playlist_filename_empty() {
        assert_eq!(playlist_filename(""), "playlist.m3u");
        assert_eq!(playlist_filename("   "), "playlist.m3u");
    }

    #[test]
    fn test_playlist_filename_already_has_extension() {
        assert_eq!(playlist_filename("test.m3u"), "test.m3u");
        assert_eq!(playlist_filename("test.m3u8"), "test.m3u8");
    }

    #[test]
    fn test_playlist_save_and_load() {
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_playlist.m3u");

        // Clean up if exists
        let _ = fs::remove_file(&temp_file);

        let mut playlist = Playlist::new("Test".to_string(), temp_file.clone());
        playlist.add_track(PathBuf::from("/music/track1.mp3"));
        playlist.add_track(PathBuf::from("/music/track2.mp3"));

        // Save
        playlist.save().unwrap();

        // Load
        let loaded = Playlist::load(&temp_file).unwrap();
        assert_eq!(loaded.name, "test_playlist");
        assert_eq!(loaded.tracks.len(), 2);
        assert_eq!(loaded.tracks[0], PathBuf::from("/music/track1.mp3"));
        assert_eq!(loaded.tracks[1], PathBuf::from("/music/track2.mp3"));

        // Clean up
        let _ = fs::remove_file(&temp_file);
    }

    #[test]
    fn test_playlist_load_ignores_comments() {
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_playlist_comments.m3u");

        // Clean up if exists
        let _ = fs::remove_file(&temp_file);

        let content = "#EXTM3U\n# This is a comment\n/music/track1.mp3\n  \n/music/track2.mp3\n";
        fs::write(&temp_file, content).unwrap();

        let loaded = Playlist::load(&temp_file).unwrap();
        assert_eq!(loaded.tracks.len(), 2);
        assert_eq!(loaded.tracks[0], PathBuf::from("/music/track1.mp3"));
        assert_eq!(loaded.tracks[1], PathBuf::from("/music/track2.mp3"));

        // Clean up
        let _ = fs::remove_file(&temp_file);
    }
}
