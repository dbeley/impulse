use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq)]
pub enum FileEntry {
    Directory(PathBuf),
    AudioFile(PathBuf),
}

impl FileEntry {
    pub fn path(&self) -> &Path {
        match self {
            FileEntry::Directory(p) | FileEntry::AudioFile(p) => p,
        }
    }

    pub fn name(&self) -> String {
        self.path()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string()
    }

    pub fn is_dir(&self) -> bool {
        matches!(self, FileEntry::Directory(_))
    }
}

pub struct Browser {
    current_dir: PathBuf,
    entries: Vec<FileEntry>,
    selected: usize,
}

impl Browser {
    pub fn new(path: PathBuf) -> Self {
        let mut browser = Self {
            current_dir: path,
            entries: Vec::new(),
            selected: 0,
        };
        browser.load_entries();
        browser
    }

    pub fn load_entries(&mut self) {
        self.entries.clear();
        
        // Add parent directory entry if not at root
        if self.current_dir.parent().is_some() {
            self.entries.push(FileEntry::Directory(
                self.current_dir.parent().unwrap().to_path_buf(),
            ));
        }

        // Read directory entries
        if let Ok(entries) = fs::read_dir(&self.current_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    self.entries.push(FileEntry::Directory(path));
                } else if is_audio_file(&path) {
                    self.entries.push(FileEntry::AudioFile(path));
                }
            }
        }

        // Sort: directories first, then files, all alphabetically
        self.entries.sort_by(|a, b| {
            match (a, b) {
                (FileEntry::Directory(p1), FileEntry::Directory(p2)) => {
                    p1.file_name().cmp(&p2.file_name())
                }
                (FileEntry::Directory(_), FileEntry::AudioFile(_)) => std::cmp::Ordering::Less,
                (FileEntry::AudioFile(_), FileEntry::Directory(_)) => std::cmp::Ordering::Greater,
                (FileEntry::AudioFile(p1), FileEntry::AudioFile(p2)) => {
                    p1.file_name().cmp(&p2.file_name())
                }
            }
        });

        self.selected = 0;
    }

    pub fn enter_selected(&mut self) -> Option<PathBuf> {
        if let Some(entry) = self.entries.get(self.selected) {
            match entry {
                FileEntry::Directory(path) => {
                    self.current_dir = path.clone();
                    self.load_entries();
                    None
                }
                FileEntry::AudioFile(path) => Some(path.clone()),
            }
        } else {
            None
        }
    }

    pub fn selected_entry(&self) -> Option<&FileEntry> {
        self.entries.get(self.selected)
    }

    pub fn current_dir(&self) -> &Path {
        &self.current_dir
    }

    pub fn entries(&self) -> &[FileEntry] {
        &self.entries
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn select_next(&mut self) {
        if !self.entries.is_empty() {
            self.selected = (self.selected + 1).min(self.entries.len() - 1);
        }
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn select_first(&mut self) {
        self.selected = 0;
    }

    pub fn select_last(&mut self) {
        if !self.entries.is_empty() {
            self.selected = self.entries.len() - 1;
        }
    }

    pub fn select_index(&mut self, index: usize) {
        if index < self.entries.len() {
            self.selected = index;
        }
    }

    pub fn get_all_audio_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for entry in WalkDir::new(&self.current_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && is_audio_file(path) {
                files.push(path.to_path_buf());
            }
        }
        files.sort();
        files
    }
}

fn is_audio_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        if let Some(ext_str) = ext.to_str() {
            matches!(
                ext_str.to_lowercase().as_str(),
                "mp3" | "flac" | "ogg" | "wav" | "m4a" | "aac" | "opus" | "wma"
            )
        } else {
            false
        }
    } else {
        false
    }
}
