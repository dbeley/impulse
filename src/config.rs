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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.volume, 0.5);
        assert!(config.lastfm.is_none());
        // The paths will vary based on the system, but they should exist
        assert!(!config.music_dir.as_os_str().is_empty());
        assert!(!config.playlist_dir.as_os_str().is_empty());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config {
            music_dir: PathBuf::from("/test/music"),
            playlist_dir: PathBuf::from("/test/playlists"),
            volume: 0.8,
            lastfm: None,
        };

        let toml_string = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&toml_string).unwrap();

        assert_eq!(deserialized.music_dir, PathBuf::from("/test/music"));
        assert_eq!(deserialized.playlist_dir, PathBuf::from("/test/playlists"));
        assert_eq!(deserialized.volume, 0.8);
        assert!(deserialized.lastfm.is_none());
    }

    #[test]
    fn test_config_with_lastfm() {
        let lastfm_config = LastfmConfig {
            enabled: true,
            api_key: "test_api_key".to_string(),
            api_secret: "test_secret".to_string(),
            session_key: "test_session".to_string(),
        };

        let config = Config {
            music_dir: PathBuf::from("/test/music"),
            playlist_dir: PathBuf::from("/test/playlists"),
            volume: 0.7,
            lastfm: Some(lastfm_config.clone()),
        };

        let toml_string = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&toml_string).unwrap();

        assert!(deserialized.lastfm.is_some());
        let lastfm = deserialized.lastfm.unwrap();
        assert!(lastfm.enabled);
        assert_eq!(lastfm.api_key, "test_api_key");
        assert_eq!(lastfm.api_secret, "test_secret");
        assert_eq!(lastfm.session_key, "test_session");
    }

    #[test]
    fn test_config_deserialization_with_defaults() {
        let toml_string = r#"
            music_dir = "/custom/music"
        "#;

        let config: Config = toml::from_str(toml_string).unwrap();
        assert_eq!(config.music_dir, PathBuf::from("/custom/music"));
        assert_eq!(config.volume, 0.5); // default
        assert!(!config.playlist_dir.as_os_str().is_empty()); // default
    }

    #[test]
    fn test_default_volume() {
        assert_eq!(default_volume(), 0.5);
    }
}
