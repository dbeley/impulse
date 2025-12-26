use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

static LOGGER: Mutex<Option<Logger>> = Mutex::new(None);

#[derive(Debug)]
pub struct Logger {
    log_path: PathBuf,
}

impl Logger {
    pub fn new(log_path: PathBuf) -> Result<Self> {
        // Create log directory if it doesn't exist
        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create log directory: {}", parent.display()))?;
        }

        Ok(Self { log_path })
    }

    pub fn log(&self, message: &str) -> Result<()> {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let log_message = format!("[{}] {}\n", timestamp, message);

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .with_context(|| format!("Failed to open log file: {}", self.log_path.display()))?;

        file.write_all(log_message.as_bytes())
            .with_context(|| format!("Failed to write to log file: {}", self.log_path.display()))?;

        Ok(())
    }
}

pub fn init_logger(log_path: PathBuf) -> Result<()> {
    let logger = Logger::new(log_path)?;
    let mut global_logger = LOGGER.lock().unwrap();
    *global_logger = Some(logger);
    Ok(())
}

pub fn log(message: &str) {
    if let Ok(logger_guard) = LOGGER.lock() {
        if let Some(logger) = logger_guard.as_ref() {
            // Ignore errors during logging to avoid disrupting the application
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

        let logger = Logger::new(log_path.clone()).unwrap();
        assert_eq!(logger.log_path, log_path);
    }

    #[test]
    fn test_logger_writes_message() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");

        let logger = Logger::new(log_path.clone()).unwrap();
        logger.log("Test message").unwrap();

        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("Test message"));
        assert!(content.contains("[20")); // Year prefix
    }

    #[test]
    fn test_logger_appends_messages() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");

        let logger = Logger::new(log_path.clone()).unwrap();
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

        let logger = Logger::new(log_path.clone()).unwrap();
        logger.log("Test message").unwrap();

        assert!(log_path.exists());
    }
}
