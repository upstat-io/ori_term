//! Pane server for oriterm.
//!
//! This crate manages terminal panes: spawning shell processes,
//! reading PTY output, routing I/O, and tracking pane metadata.
//! It has no knowledge of how panes are presented — that is the
//! client's responsibility.
//!
//! # Architecture
//!
//! `oriterm_mux` sits between `oriterm_core` (terminal emulation) and
//! `oriterm` (client binary). It owns pane lifecycle state: which panes
//! exist, their PTY processes, and the event bridge between PTY reader
//! threads and the client.

#![deny(unsafe_code)]

pub mod backend;
pub mod discovery;
pub mod domain;
pub mod id;
pub mod in_process;
pub mod mux_event;
pub mod pane;
pub mod protocol;
pub mod pty;
pub mod registry;
pub mod server;
pub mod shell_integration;

pub use backend::{EmbeddedMux, MuxBackend, MuxClient};
pub use domain::SpawnConfig;
pub use id::{ClientId, DomainId, PaneId};
pub use mux_event::MuxNotification;
pub use pane::MarkCursor;
pub use protocol::{
    MuxPdu, PaneSnapshot, ProtocolCodec, WireCell, WireCellFlags, WireCursor, WireCursorShape,
    WireRgb,
};
