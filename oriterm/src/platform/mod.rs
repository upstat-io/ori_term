//! Platform-specific abstractions for cross-platform operations.
//!
//! Each submodule provides a unified API with `#[cfg]`-gated platform
//! implementations. Follows Chromium's pattern of thin platform glue
//! behind a shared interface.

pub mod config_paths;
pub mod shutdown;
pub mod url;
