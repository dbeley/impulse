use anyhow::Result;
use std::fs::File;
use std::path::Path;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::{MetadataOptions, StandardTagKey, Tag};
use symphonia::core::probe::Hint;

#[derive(Debug, Clone, Default)]
pub struct TrackMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub year: Option<String>,
    pub genre: Option<String>,
    pub track_number: Option<String>,
    pub disc_number: Option<String>,
    pub duration_secs: Option<u64>,
    pub cover_art: Option<Vec<u8>>,
}

impl TrackMetadata {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let file = File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());

        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(extension);
        }

        let mut probed = symphonia::default::get_probe().format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions {
                limit_metadata_bytes: symphonia::core::meta::Limit::Maximum(usize::MAX),
                limit_visual_bytes: symphonia::core::meta::Limit::Maximum(usize::MAX),
            },
        )?;

        let mut metadata = TrackMetadata::default();

        // Get duration if available
        if let Some(track) = probed.format.default_track() {
            if let Some(time_base) = track.codec_params.time_base {
                if let Some(n_frames) = track.codec_params.n_frames {
                    let duration_secs = time_base.calc_time(n_frames).seconds;
                    metadata.duration_secs = Some(duration_secs);
                }
            }
        }

        // Read metadata from the format reader - this gets ID3v2 tags from MP3 files
        let mut format = probed.format;
        if let Some(metadata_rev) = format.metadata().current() {
            metadata.read_tags(metadata_rev.tags());
            metadata.read_visuals(metadata_rev.visuals());
        }

        // Also check for metadata in the probed metadata (ID3v1 tags)
        if let Some(probed_metadata) = probed.metadata.get() {
            if let Some(metadata_rev) = probed_metadata.current() {
                metadata.read_tags(metadata_rev.tags());
                metadata.read_visuals(metadata_rev.visuals());
            }
        }

        // If no embedded cover art, look for external image files
        if metadata.cover_art.is_none() {
            metadata.cover_art = Self::find_external_cover_art(path);
        }

        Ok(metadata)
    }

    fn read_tags(&mut self, tags: &[Tag]) {
        for tag in tags {
            if let Some(std_key) = tag.std_key {
                let value_str = tag.value.to_string();

                match std_key {
                    StandardTagKey::TrackTitle => self.title = Some(value_str),
                    StandardTagKey::Artist => self.artist = Some(value_str),
                    StandardTagKey::Album => self.album = Some(value_str),
                    StandardTagKey::AlbumArtist => self.album_artist = Some(value_str),
                    StandardTagKey::Date | StandardTagKey::ReleaseDate => {
                        self.year = Some(value_str);
                    }
                    StandardTagKey::Genre => self.genre = Some(value_str),
                    StandardTagKey::TrackNumber => self.track_number = Some(value_str),
                    StandardTagKey::DiscNumber => self.disc_number = Some(value_str),
                    _ => {}
                }
            }
        }
    }

    fn read_visuals(&mut self, visuals: &[symphonia::core::meta::Visual]) {
        // Get the first visual (usually the cover art)
        if let Some(visual) = visuals.first() {
            self.cover_art = Some(visual.data.to_vec());
        }
    }

    fn find_external_cover_art(audio_path: &Path) -> Option<Vec<u8>> {
        // Get the directory containing the audio file
        let dir = audio_path.parent()?;

        // Common cover art filenames (case-insensitive)
        let cover_names = [
            "cover.jpg",
            "cover.jpeg",
            "cover.png",
            "folder.jpg",
            "folder.jpeg",
            "folder.png",
            "album.jpg",
            "album.jpeg",
            "album.png",
            "front.jpg",
            "front.jpeg",
            "front.png",
            "albumart.jpg",
            "albumart.jpeg",
            "albumart.png",
            "Cover.jpg",
            "Cover.jpeg",
            "Cover.png",
            "Folder.jpg",
            "Folder.jpeg",
            "Folder.png",
            "Album.jpg",
            "Album.jpeg",
            "Album.png",
        ];

        // Try each common filename
        for name in &cover_names {
            let cover_path = dir.join(name);
            if cover_path.exists() && cover_path.is_file() {
                if let Ok(data) = std::fs::read(&cover_path) {
                    return Some(data);
                }
            }
        }

        // If no exact match, try finding any image file in the directory
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        let ext_lower = ext.to_lowercase();
                        if matches!(ext_lower.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp") {
                            if let Ok(data) = std::fs::read(&path) {
                                return Some(data);
                            }
                        }
                    }
                }
            }
        }

        None
    }

    pub fn format_duration(&self) -> String {
        match self.duration_secs {
            Some(secs) => {
                let hours = secs / 3600;
                let minutes = (secs % 3600) / 60;
                let seconds = secs % 60;

                if hours > 0 {
                    format!("{hours}:{minutes:02}:{seconds:02}")
                } else {
                    format!("{minutes}:{seconds:02}")
                }
            }
            None => "Unknown".to_string(),
        }
    }
}
