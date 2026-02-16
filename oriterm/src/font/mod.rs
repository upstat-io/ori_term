//! Font management: discovery, loading, and rasterization.
//!
//! This module handles finding font files on disk across platforms, loading
//! them into memory, and rasterizing glyphs for the GPU renderer.

pub mod discovery;
