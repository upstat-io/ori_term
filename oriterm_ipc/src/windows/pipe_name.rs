//! Named pipe name construction and wide-string helpers.
//!
//! Windows named pipes live in the `\\.\pipe\` namespace. We include the
//! username to avoid collisions between users on the same machine.

use std::path::{Path, PathBuf};

/// Build the default named pipe path for the daemon.
///
/// Format: `\\.\pipe\oriterm-mux-{USERNAME}`
pub fn pipe_name() -> PathBuf {
    let user = std::env::var("USERNAME").unwrap_or_else(|_| String::from("unknown"));
    PathBuf::from(format!(r"\\.\pipe\oriterm-mux-{user}"))
}

/// Convert a `Path` to a null-terminated wide string for Win32 APIs.
pub(crate) fn to_wide_string(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
