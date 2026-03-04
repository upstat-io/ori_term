//! Multiplexer for oriterm.
//!
//! This crate provides the complete multiplexing layer: data structures
//! (identity types, split trees, layout computation), PTY management,
//! the in-process mux orchestrator, and the event bridge between PTY
//! reader threads and the GUI.
//!
//! # Architecture
//!
//! `oriterm_mux` sits between `oriterm_core` (terminal emulation) and
//! `oriterm` (GUI binary). It owns all multiplexing state: which panes
//! exist, how they're arranged, how to navigate between them, and the
//! PTY processes that back each pane.

#![deny(unsafe_code)]

pub mod backend;
pub mod discovery;
pub mod domain;
pub mod id;
pub mod in_process;
pub mod layout;
pub mod mux_event;
pub mod nav;
pub mod pane;
pub mod protocol;
pub mod pty;
pub mod registry;
pub mod server;
pub mod session;
pub mod shell_integration;

pub use backend::{EmbeddedMux, MuxBackend, MuxClient};
pub use domain::{Domain, DomainState, SpawnConfig};
pub use id::{ClientId, DomainId, IdAllocator, MuxId, PaneId, SessionId, TabId, WindowId};
pub use in_process::{ClosePaneResult, InProcessMux};
pub use layout::{SplitDirection, SplitTree};
pub use mux_event::{MuxEvent, MuxEventProxy, MuxNotification};
pub use nav::Direction;
pub use pane::{MarkCursor, Pane};
pub use protocol::{
    DecodeError, DecodedFrame, FrameHeader, MsgType, MuxPdu, MuxTabInfo, MuxWindowInfo,
    PaneSnapshot, ProtocolCodec, WireCell, WireCellFlags, WireColor, WireCursor, WireCursorShape,
    WireRgb, WireSearchMatch, WireSelection,
};
pub use pty::{ExitStatus, PtyConfig, PtyControl, PtyHandle, spawn_pty};
pub use registry::{PaneEntry, PaneRegistry, SessionRegistry};
pub use session::{MuxTab, MuxWindow};
