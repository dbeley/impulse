use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq)]
pub enum FileEntry {
    Directory(PathBuf),
    ParentDirectory(PathBuf),
    AudioFile(PathBuf),
}

impl FileEntry {
    pub fn path(&self) -> &Path {
        match self {
            FileEntry::Directory(p) | FileEntry::ParentDirectory(p) | FileEntry::AudioFile(p) => p,
        }
    }

    pub fn name(&self) -> String {
        match self {
            FileEntry::ParentDirectory(_) => "..".to_string(),
            _ => self
                .path()
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string(),
        }
    }

    pub fn is_dir(&self) -> bool {
        matches!(
            self,
            FileEntry::Directory(_) | FileEntry::ParentDirectory(_)
        )
    }
}

pub struct Browser {
    current_dir: PathBuf,
    entries: Vec<FileEntry>,
    selected: usize,
    last_selected: HashMap<PathBuf, usize>,
}

impl Browser {
    pub fn new(path: PathBuf) -> Self {
        let mut browser = Self {
            current_dir: path,
            entries: Vec::new(),
            selected: 0,
            last_selected: HashMap::new(),
        };
        browser.load_entries();
        browser
    }

    pub fn load_entries(&mut self) {
        self.entries.clear();

        let has_parent = self.current_dir.parent().is_some();

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
        self.entries.sort_by(|a, b| match (a, b) {
            (FileEntry::Directory(_), FileEntry::AudioFile(_)) => std::cmp::Ordering::Less,
            (FileEntry::AudioFile(_), FileEntry::Directory(_)) => std::cmp::Ordering::Greater,
            (FileEntry::Directory(p1), FileEntry::Directory(p2))
            | (FileEntry::AudioFile(p1), FileEntry::AudioFile(p2)) => {
                p1.file_name().cmp(&p2.file_name())
            }
            _ => std::cmp::Ordering::Equal,
        });

        // Add parent directory entry at the beginning if not at root
        if has_parent {
            self.entries.insert(
                0,
                FileEntry::ParentDirectory(self.current_dir.parent().unwrap().to_path_buf()),
            );
        }

        let preferred = self
            .last_selected
            .get(&self.current_dir)
            .copied()
            .unwrap_or(0);

        self.selected = if self.entries.is_empty() {
            0
        } else if preferred >= self.entries.len() {
            self.entries.len() - 1
        } else {
            preferred
        };
    }

    pub fn enter_selected(&mut self) -> Option<PathBuf> {
        if let Some(entry) = self.entries.get(self.selected).cloned() {
            match entry {
                FileEntry::Directory(path) | FileEntry::ParentDirectory(path) => {
                    self.remember_selection();
                    self.current_dir.clone_from(&path);
                    self.load_entries();
                    None
                }
                FileEntry::AudioFile(path) => {
                    self.remember_selection();
                    Some(path)
                }
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

    #[allow(dead_code)]
    pub fn navigate_to(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.remember_selection();
            self.current_dir = path;
            self.load_entries();
        }
    }

    pub fn go_parent(&mut self) {
        if let Some(parent) = self.current_dir.parent().map(std::path::Path::to_path_buf) {
            self.remember_selection();
            self.current_dir = parent;
            self.load_entries();
        }
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

    pub fn select_entry_by_path(&mut self, target: &Path) {
        if let Some((idx, _)) = self
            .entries
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.path() == target)
        {
            self.selected = idx;
        }
    }

    fn remember_selection(&mut self) {
        self.last_selected
            .insert(self.current_dir.clone(), self.selected);
    }

    pub fn get_all_audio_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for entry in WalkDir::new(&self.current_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(std::result::Result::ok)
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

pub fn is_audio_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        if let Some(ext_str) = ext.to_str() {
            matches!(
                ext_str.to_lowercase().as_str(),
                "mp3" | "flac" | "ogg" | "wav" | "m4a" | "aac" | "alac" | "opus"
            )
        } else {
            false
        }
    } else {
        false
    }
}
