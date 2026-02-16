//! System color theme for adapting terminal appearance.
//!
//! Represents the operating system's dark/light mode preference. Used at
//! startup to select the default palette, and can be overridden by user
//! configuration.

/// Operating system color theme preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Theme {
    /// Dark background, light text (most terminal emulators' default).
    Dark,
    /// Light background, dark text.
    Light,
    /// System preference could not be determined.
    Unknown,
}

impl Theme {
    /// Whether this theme prefers a dark background.
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark | Self::Unknown)
    }
}

impl Default for Theme {
    /// Defaults to `Dark` — the conventional terminal default.
    fn default() -> Self {
        Self::Dark
    }
}

#[cfg(test)]
mod tests;
