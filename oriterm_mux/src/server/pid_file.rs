//! PID file management for the mux daemon.
//!
//! The [`PidFile`] guard writes the current process ID to a known path on
//! creation and removes it on drop, ensuring cleanup even on panic.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// RAII guard for a PID file. Removes the file on drop.
pub struct PidFile {
    /// Path to the PID file (removed on drop).
    path: PathBuf,
}

impl PidFile {
    /// Create a PID file at the default path, writing the current process ID.
    pub fn create() -> io::Result<Self> {
        Self::create_at(&pid_file_path())
    }

    /// Create at a specific path (for testing).
    pub fn create_at(path: &Path) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut f = std::fs::File::create(path)?;
        write!(f, "{}", std::process::id())?;
        Ok(Self {
            path: path.to_owned(),
        })
    }

    /// Path to the PID file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PidFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Default PID file path.
///
/// Delegates to `oriterm_ipc::pid_file_path()` for platform-appropriate
/// path selection.
pub fn pid_file_path() -> PathBuf {
    oriterm_ipc::pid_file_path()
}

/// Read the PID from an existing PID file.
pub fn read_pid(path: &Path) -> io::Result<u32> {
    let content = std::fs::read_to_string(path)?;
    content
        .trim()
        .parse::<u32>()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
