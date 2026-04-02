use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

static LOGGER: Mutex<Option<Logger>> = Mutex::new(None);

#[derive(Debug)]
pub struct Logger {
    file: std::fs::File,
}

impl Logger {
    pub fn new(log_path: &Path) -> Result<Self> {
        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create log directory: {}", parent.display()))?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .with_context(|| format!("Failed to open log file: {}", log_path.display()))?;

        Ok(Self { file })
    }

    pub fn log(&mut self, message: &str) -> Result<()> {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let log_message = format!("[{timestamp}] {message}\n");

        self.file
            .write_all(log_message.as_bytes())
            .with_context(|| "Failed to write to log file")?;

        Ok(())
    }
}

pub fn init_logger(log_path: &Path) -> Result<()> {
    let logger = Logger::new(log_path)?;
    let mut global_logger = LOGGER.lock().unwrap();
    *global_logger = Some(logger);
    Ok(())
}

pub fn log(message: &str) {
    if let Ok(mut logger_guard) = LOGGER.lock() {
        if let Some(logger) = logger_guard.as_mut() {
            let _ = logger.log(message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_logger_creation() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");

        let logger = Logger::new(&log_path).unwrap();
        assert!(logger.file.metadata().is_ok());
    }

    #[test]
    fn test_logger_writes_message() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");

        let mut logger = Logger::new(&log_path).unwrap();
        logger.log("Test message").unwrap();

        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("Test message"));
        assert!(content.contains("[20"));
    }

    #[test]
    fn test_logger_appends_messages() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");

        let mut logger = Logger::new(&log_path).unwrap();
        logger.log("First message").unwrap();
        logger.log("Second message").unwrap();

        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("First message"));
        assert!(content.contains("Second message"));
    }

    #[test]
    fn test_logger_creates_directory() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("logs").join("test.log");

        let mut logger = Logger::new(&log_path).unwrap();
        logger.log("Test message").unwrap();

        assert!(log_path.exists());
    }
}
