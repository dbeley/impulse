use anyhow::{Context, Result};
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
    #[serde(default)]
    pub lastfm: Option<LastfmConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LastfmConfig {
    pub enabled: bool,
    pub api_key: String,
    pub api_secret: String,
    pub session_key: String,
}

fn default_music_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Music")
}

fn default_playlist_dir() -> PathBuf {
    default_data_dir().join("playlists")
}

fn default_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("impulse")
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
            lastfm: None,
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
            let mut config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&mut self) -> Result<()> {
        let config_path = Self::config_path();

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory {}", parent.display())
            })?;
        }

        // Create playlist directory if it doesn't exist. If the configured path
        // cannot be created (e.g. placeholder /home/user), fall back to the
        // default directory so saving the config (and the session key) still
        // succeeds.
        if !self.playlist_dir.exists() {
            if let Err(err) = fs::create_dir_all(&self.playlist_dir) {
                let fallback = default_playlist_dir();
                eprintln!(
                    "Warning: failed to create playlist directory at {}: {}. Falling back to {}",
                    self.playlist_dir.display(),
                    err,
                    fallback.display()
                );
                fs::create_dir_all(&fallback).with_context(|| {
                    format!(
                        "Failed to create fallback playlist directory at {}",
                        fallback.display()
                    )
                })?;
                self.playlist_dir = fallback;
            }
        }

        let content = toml::to_string_pretty(self)?;
        fs::write(&config_path, content)
            .with_context(|| format!("Failed to write config file at {}", config_path.display()))?;
        Ok(())
    }

    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("impulse")
            .join("impulse.conf")
    }
}
