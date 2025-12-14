use anyhow::{Context, Result};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Player {
    sink: Arc<Mutex<Option<Sink>>>,
    _stream: Arc<OutputStream>,
    stream_handle: Arc<OutputStreamHandle>,
    current_track: Arc<Mutex<Option<PathBuf>>>,
}

impl Player {
    pub fn new() -> Result<Self> {
        let (stream, stream_handle) = OutputStream::try_default()
            .context("Failed to create audio output stream")?;
        
        Ok(Self {
            sink: Arc::new(Mutex::new(None)),
            _stream: Arc::new(stream),
            stream_handle: Arc::new(stream_handle),
            current_track: Arc::new(Mutex::new(None)),
        })
    }

    pub fn play(&self, path: PathBuf) -> Result<()> {
        let file = File::open(&path)
            .context(format!("Failed to open audio file: {:?}", path))?;

        let file_metadata = file.metadata()
            .context(format!("Failed to read file metadata: {:?}", path))?;

        if file_metadata.len() == 0 {
            anyhow::bail!("Audio file is empty: {:?}", path);
        }

        let source = Decoder::new(BufReader::new(file))
            .context(format!("Failed to decode audio file: {:?}. The file may be corrupted, incomplete, or in an unsupported format.", path))?;

        let sink = Sink::try_new(&self.stream_handle)
            .context("Failed to create audio sink")?;

        sink.append(source);

        *self.sink.lock().unwrap() = Some(sink);
        *self.current_track.lock().unwrap() = Some(path);

        Ok(())
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

    pub fn set_volume(&self, volume: f32) {
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            sink.set_volume(volume);
        }
    }
}
