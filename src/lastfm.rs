use crate::config::LastfmConfig;
use crate::metadata::TrackMetadata;
use anyhow::Result;
use std::path::Path;

#[cfg(feature = "lastfm")]
use anyhow::Context;
#[cfg(feature = "lastfm")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "lastfm")]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "lastfm")]
use rustfm_scrobble::{Scrobble, Scrobbler};

pub struct LastfmScrobbler {
    #[cfg(feature = "lastfm")]
    scrobbler: Arc<Mutex<Option<Scrobbler>>>,
    enabled: bool,
    #[cfg(feature = "lastfm")]
    current_track_start: Arc<Mutex<Option<u64>>>,
}

impl LastfmScrobbler {
    pub fn new(_config: Option<&LastfmConfig>) -> Result<Self> {
        #[cfg(feature = "lastfm")]
        {
            if let Some(cfg) = _config {
                if cfg.enabled {
                    let mut scrobbler =
                        Scrobbler::new(&cfg.api_key, &cfg.api_secret);
                    scrobbler.authenticate_with_session_key(&cfg.session_key);
                    return Ok(Self {
                        scrobbler: Arc::new(Mutex::new(Some(scrobbler))),
                        enabled: true,
                        current_track_start: Arc::new(Mutex::new(None)),
                    });
                }
            }
            Ok(Self {
                scrobbler: Arc::new(Mutex::new(None)),
                enabled: false,
                current_track_start: Arc::new(Mutex::new(None)),
            })
        }

        #[cfg(not(feature = "lastfm"))]
        {
            Ok(Self { enabled: false })
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[cfg(feature = "lastfm")]
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
            .map(|s| s.to_string())
            .or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "Unknown Track".to_string());

        let album = metadata.album.as_deref().unwrap_or("");

        (artist, title, album)
    }

    #[cfg(feature = "lastfm")]
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

    #[cfg(not(feature = "lastfm"))]
    pub fn now_playing(&self, _path: &Path, _metadata: &TrackMetadata) -> Result<()> {
        Ok(())
    }

    #[cfg(feature = "lastfm")]
    pub fn scrobble(&self, path: &Path, metadata: &TrackMetadata) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let scrobbler_guard = self.scrobbler.lock().unwrap();
        if let Some(scrobbler) = scrobbler_guard.as_ref() {
            let timestamp = self.current_track_start.lock().unwrap().take();

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

    #[cfg(not(feature = "lastfm"))]
    pub fn scrobble(&self, _path: &Path, _metadata: &TrackMetadata) -> Result<()> {
        Ok(())
    }

    pub fn clear_current_track(&self) {
        #[cfg(feature = "lastfm")]
        {
            *self.current_track_start.lock().unwrap() = None;
        }
    }
}
