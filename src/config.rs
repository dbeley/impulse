use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_music_dir")]
    pub music_dir: PathBuf,
    #[serde(default = "default_playlist_dir")]
    pub playlist_dir: PathBuf,
    #[serde(default = "default_volume")]
    pub volume: f32,
}

fn default_music_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Music")
}

fn default_playlist_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("impulse")
        .join("playlists")
}

fn default_volume() -> f32 {
    0.5
}

impl Default for Config {
    fn default() -> Self {
        Self {
            music_dir: default_music_dir(),
            playlist_dir: default_playlist_dir(),
            volume: default_volume(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path();
        
        if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path();
        
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Create playlist directory if it doesn't exist
        if !self.playlist_dir.exists() {
            fs::create_dir_all(&self.playlist_dir)?;
        }

        let content = toml::to_string_pretty(self)?;
        fs::write(&config_path, content)?;
        Ok(())
    }

    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("impulse")
            .join("config.toml")
    }
}
