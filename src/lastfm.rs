use crate::config::LastfmConfig;
use crate::metadata::TrackMetadata;
use anyhow::{Context, Result};
use rustfm_scrobble::{Scrobble, Scrobbler};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct LastfmScrobbler {
    scrobbler: Arc<Mutex<Option<Scrobbler>>>,
    enabled: bool,
    current_track_start: Arc<Mutex<Option<u64>>>,
}

impl LastfmScrobbler {
    pub fn new(config: Option<&LastfmConfig>) -> Self {
        if let Some(cfg) = config {
            if cfg.enabled {
                let mut scrobbler = Scrobbler::new(&cfg.api_key, &cfg.api_secret);
                scrobbler.authenticate_with_session_key(&cfg.session_key);
                return Self {
                    scrobbler: Arc::new(Mutex::new(Some(scrobbler))),
                    enabled: true,
                    current_track_start: Arc::new(Mutex::new(None)),
                };
            }
        }
        Self {
            scrobbler: Arc::new(Mutex::new(None)),
            enabled: false,
            current_track_start: Arc::new(Mutex::new(None)),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn extract_track_info<'a>(
        path: &'a Path,
        metadata: &'a TrackMetadata,
    ) -> (&'a str, String, &'a str) {
        let artist = metadata
            .artist
            .as_deref()
            .or(metadata.album_artist.as_deref())
            .unwrap_or("Unknown Artist");

        let title = metadata
            .title
            .as_deref()
            .map(std::string::ToString::to_string)
            .or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(std::string::ToString::to_string)
            })
            .unwrap_or_else(|| "Unknown Track".to_string());

        let album = metadata.album.as_deref().unwrap_or("");

        (artist, title, album)
    }

    pub fn now_playing(&self, path: &Path, metadata: &TrackMetadata) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let scrobbler_guard = self.scrobbler.lock().unwrap();
        if let Some(scrobbler) = scrobbler_guard.as_ref() {
            let (artist, title, album) = Self::extract_track_info(path, metadata);

            let scrobble = Scrobble::new(artist, &title, album);

            // Update now playing on Last.fm
            scrobbler
                .now_playing(&scrobble)
                .context("Failed to update now playing on Last.fm")?;

            // Store the track start time for later scrobbling
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            *self.current_track_start.lock().unwrap() = Some(timestamp);
        }

        Ok(())
    }

    pub fn scrobble(&self, path: &Path, metadata: &TrackMetadata) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let scrobbler_guard = self.scrobbler.lock().unwrap();
        if let Some(scrobbler) = scrobbler_guard.as_ref() {
            let timestamp = *self.current_track_start.lock().unwrap();

            if let Some(ts) = timestamp {
                let (artist, title, album) = Self::extract_track_info(path, metadata);

                let mut scrobble = Scrobble::new(artist, &title, album);
                scrobble.with_timestamp(ts);

                // Submit the scrobble to Last.fm
                scrobbler
                    .scrobble(&scrobble)
                    .context("Failed to scrobble track to Last.fm")?;
            }
        }

        Ok(())
    }

    pub fn clear_current_track(&self) {
        *self.current_track_start.lock().unwrap() = None;
    }
}
