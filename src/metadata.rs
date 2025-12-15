use anyhow::Result;
use std::fs::File;
use std::path::Path;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
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
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(extension);
        }

        let probed = symphonia::default::get_probe().format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
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

        // Read metadata from the format reader
        let mut format = probed.format;
        if let Some(metadata_rev) = format.metadata().current() {
            metadata.read_tags(metadata_rev.tags());
            metadata.read_visuals(metadata_rev.visuals());
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
                        self.year = Some(value_str)
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

    pub fn format_duration(&self) -> String {
        match self.duration_secs {
            Some(secs) => {
                let hours = secs / 3600;
                let minutes = (secs % 3600) / 60;
                let seconds = secs % 60;

                if hours > 0 {
                    format!("{}:{:02}:{:02}", hours, minutes, seconds)
                } else {
                    format!("{}:{:02}", minutes, seconds)
                }
            }
            None => "Unknown".to_string(),
        }
    }
}
