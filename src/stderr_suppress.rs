use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd};

/// Redirects stderr to /dev/null to suppress ALSA and other C library errors
/// that would otherwise corrupt the TUI display.
///
/// Returns the original stderr file descriptor so it can be restored later if needed.
pub fn suppress_stderr() -> Option<File> {
    unsafe {
        // Duplicate the current stderr fd so we can restore it later
        let stderr_fd = libc::dup(libc::STDERR_FILENO);
        if stderr_fd < 0 {
            return None;
        }

        // Open /dev/null
        let devnull = libc::open(
            b"/dev/null\0".as_ptr() as *const libc::c_char,
            libc::O_WRONLY,
        );
        if devnull < 0 {
            libc::close(stderr_fd);
            return None;
        }

        // Redirect stderr to /dev/null
        if libc::dup2(devnull, libc::STDERR_FILENO) < 0 {
            libc::close(devnull);
            libc::close(stderr_fd);
            return None;
        }

        libc::close(devnull);

        Some(File::from_raw_fd(stderr_fd))
    }
}

/// Restores stderr from a previously saved file descriptor.
pub fn restore_stderr(original_stderr: File) {
    unsafe {
        let fd = original_stderr.as_raw_fd();
        libc::dup2(fd, libc::STDERR_FILENO);
        // File will be closed when dropped
    }
}
