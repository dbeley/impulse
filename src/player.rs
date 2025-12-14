use crate::metadata::TrackMetadata;
use anyhow::{Context, Result};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CodecRegistry, DecoderOptions};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia_adapter_libopus::OpusDecoder;

#[derive(Clone)]
pub struct Player {
    sink: Arc<Mutex<Option<Sink>>>,
    _stream: Rc<OutputStream>,
    stream_handle: Arc<OutputStreamHandle>,
    current_track: Arc<Mutex<Option<PathBuf>>>,
    current_metadata: Arc<Mutex<Option<TrackMetadata>>>,
}

impl Player {
    pub fn new() -> Result<Self> {
        let (stream, stream_handle) =
            OutputStream::try_default().context("Failed to create audio output stream")?;

        Ok(Self {
            sink: Arc::new(Mutex::new(None)),
            _stream: Rc::new(stream),
            stream_handle: Arc::new(stream_handle),
            current_track: Arc::new(Mutex::new(None)),
            current_metadata: Arc::new(Mutex::new(None)),
        })
    }

    pub fn play(&self, path: PathBuf) -> Result<()> {
        let file = File::open(&path).context(format!("Failed to open audio file: {:?}", path))?;

        let file_metadata = file
            .metadata()
            .context(format!("Failed to read file metadata: {:?}", path))?;

        if file_metadata.len() == 0 {
            anyhow::bail!("Audio file is empty: {:?}", path);
        }

        // Try standard rodio decoder first
        let decode_result = Decoder::new(BufReader::new(file));

        let source: Box<dyn Source<Item = i16> + Send> = match decode_result {
            Ok(decoder) => Box::new(decoder),
            Err(_) => {
                // If standard decoder fails, try Opus decoder for .opus files
                if path.extension().and_then(|e| e.to_str()) == Some("opus") {
                    let opus_source = self
                        .decode_opus(&path)
                        .context(format!("Failed to decode Opus file: {:?}", path))?;
                    Box::new(opus_source)
                } else {
                    // Re-open file and try again to get proper error
                    let file = File::open(&path)?;
                    Decoder::new(BufReader::new(file))
                        .context(format!("Failed to decode audio file: {:?}. The file may be corrupted, incomplete, or in an unsupported format.", path))?;
                    unreachable!()
                }
            }
        };

        let sink = Sink::try_new(&self.stream_handle).context("Failed to create audio sink")?;

        sink.append(source);

        // Read metadata from the track
        let metadata = TrackMetadata::from_file(&path).ok();

        *self.sink.lock().unwrap() = Some(sink);
        *self.current_track.lock().unwrap() = Some(path);
        *self.current_metadata.lock().unwrap() = metadata;

        Ok(())
    }

    fn decode_opus(&self, path: &PathBuf) -> Result<impl Source<Item = i16> + Send> {
        // Create a media source stream from the file
        let file = File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        // Create a hint to help the format registry
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(extension);
        }

        // Create a custom codec registry with Opus support
        let mut codecs = CodecRegistry::new();
        codecs.register_all::<OpusDecoder>();

        // Probe the media source
        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                mss,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .context("Failed to probe Opus file")?;

        let format = probed.format;
        let track = format
            .default_track()
            .context("No default track found in Opus file")?;

        let track_id = track.id;
        let codec_params = track.codec_params.clone();

        // Create decoder
        let decoder = codecs
            .make(&codec_params, &DecoderOptions::default())
            .context("Failed to create Opus decoder")?;

        Ok(SymphoniaSource::new(decoder, format, track_id))
    }

    pub fn pause(&self) {
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            sink.pause();
        }
    }

    pub fn resume(&self) {
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            sink.play();
        }
    }

    pub fn stop(&self) {
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            sink.stop();
        }
        *self.sink.lock().unwrap() = None;
        *self.current_track.lock().unwrap() = None;
        *self.current_metadata.lock().unwrap() = None;
    }

    pub fn is_playing(&self) -> bool {
        self.sink
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| !s.is_paused() && !s.empty())
            .unwrap_or(false)
    }

    pub fn is_paused(&self) -> bool {
        self.sink
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.is_paused())
            .unwrap_or(false)
    }

    pub fn is_finished(&self) -> bool {
        self.sink
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.empty())
            .unwrap_or(true)
    }

    pub fn current_track(&self) -> Option<PathBuf> {
        self.current_track.lock().unwrap().clone()
    }

    pub fn current_metadata(&self) -> Option<TrackMetadata> {
        self.current_metadata.lock().unwrap().clone()
    }

    pub fn set_volume(&self, volume: f32) {
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            sink.set_volume(volume);
        }
    }
}

// Custom source that wraps Symphonia decoder for use with rodio
struct SymphoniaSource {
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
    format: Box<dyn symphonia::core::formats::FormatReader>,
    track_id: u32,
    sample_buf: Option<SampleBuffer<i16>>,
    sample_pos: usize,
    channels: u16,
    sample_rate: u32,
}

impl SymphoniaSource {
    fn new(
        decoder: Box<dyn symphonia::core::codecs::Decoder>,
        format: Box<dyn symphonia::core::formats::FormatReader>,
        track_id: u32,
    ) -> Self {
        let codec_params = &decoder.codec_params();
        let channels = codec_params.channels.map(|c| c.count() as u16).unwrap_or(2);
        let sample_rate = codec_params.sample_rate.unwrap_or(48000);

        Self {
            decoder,
            format,
            track_id,
            sample_buf: None,
            sample_pos: 0,
            channels,
            sample_rate,
        }
    }

    fn decode_next_packet(&mut self) -> Option<()> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(_) => return None,
            };

            if packet.track_id() != self.track_id {
                continue;
            }

            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    if self.sample_buf.is_none() {
                        let spec = *decoded.spec();
                        let duration = decoded.capacity() as u64;
                        self.sample_buf = Some(SampleBuffer::new(duration, spec));
                    }

                    if let Some(buf) = &mut self.sample_buf {
                        buf.copy_interleaved_ref(decoded);
                        self.sample_pos = 0;
                        return Some(());
                    }
                }
                Err(_) => continue,
            }
        }
    }
}

impl Iterator for SymphoniaSource {
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(buf) = &self.sample_buf {
                if self.sample_pos < buf.len() {
                    let sample = buf.samples()[self.sample_pos];
                    self.sample_pos += 1;
                    return Some(sample);
                }
            }

            self.decode_next_packet()?;
        }
    }
}

impl Source for SymphoniaSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}
