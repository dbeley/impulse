use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Playlist {
    pub name: String,
    pub path: PathBuf,
    pub tracks: Vec<PathBuf>,
}

impl Playlist {
    pub fn new(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            tracks: Vec::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .context(format!("Failed to read playlist: {:?}", path))?;
        
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

    pub fn add_track(&mut self, track: PathBuf) {
        self.tracks.push(track);
    }

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

    pub fn create_playlist(&mut self, name: String) -> Result<()> {
        let filename = format!("{}.m3u", name.replace('/', "_").replace('\\', "_"));
        let path = self.playlist_dir.join(filename);
        
        let playlist = Playlist::new(name, path);
        playlist.save()?;
        self.playlists.push(playlist);
        
        Ok(())
    }

    pub fn delete_playlist(&mut self, index: usize) -> Result<()> {
        if index < self.playlists.len() {
            let playlist = &self.playlists[index];
            fs::remove_file(&playlist.path)
                .context(format!("Failed to delete playlist: {:?}", playlist.path))?;
            self.playlists.remove(index);
        }
        Ok(())
    }

    pub fn playlists(&self) -> &[Playlist] {
        &self.playlists
    }

    pub fn playlists_mut(&mut self) -> &mut [Playlist] {
        &mut self.playlists
    }

    pub fn get_playlist(&self, index: usize) -> Option<&Playlist> {
        self.playlists.get(index)
    }

    pub fn get_playlist_mut(&mut self, index: usize) -> Option<&mut Playlist> {
        self.playlists.get_mut(index)
    }
}
