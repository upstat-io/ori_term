//! Local domain — spawns shells on the local machine via `portable-pty`.

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::mpsc;

use oriterm_core::{FairMutex, Term, Theme};

use crate::{DomainId, PaneId};

use super::{Domain, DomainState, SpawnConfig};

use crate::mux_event::{MuxEvent, MuxEventProxy};
use crate::pane::{Pane, PaneNotifier, PaneParts};
use crate::pty::{PtyConfig, PtyEventLoop, spawn_pty};

/// Spawns shells on the local machine.
///
/// The simplest domain — creates a PTY via `portable-pty`, wires up
/// `MuxEventProxy` as the event listener, and returns a fully assembled
/// `Pane`.
pub struct LocalDomain {
    /// Domain identity.
    id: DomainId,
    /// Lifecycle state.
    state: DomainState,
}

impl Domain for LocalDomain {
    fn id(&self) -> DomainId {
        self.id
    }

    #[allow(
        clippy::unnecessary_literal_bound,
        reason = "trait signature requires &str, literal 'local' is always valid"
    )]
    fn name(&self) -> &str {
        "local"
    }

    fn state(&self) -> DomainState {
        self.state
    }

    fn can_spawn(&self) -> bool {
        self.state == DomainState::Attached
    }
}

impl LocalDomain {
    /// Create a new local domain.
    pub fn new(id: DomainId) -> Self {
        Self {
            id,
            state: DomainState::Attached,
        }
    }

    /// Spawn a new pane with a live shell process.
    ///
    /// Creates the PTY, terminal state machine, reader thread, and wires
    /// the `MuxEventProxy` event listener. Returns a fully assembled `Pane`.
    #[allow(
        clippy::too_many_arguments,
        reason = "all six parameters are required to assemble a Pane; \
                  grouped into a struct when Section 31 wires this into App"
    )]
    pub fn spawn_pane(
        &self,
        pane_id: PaneId,
        config: &SpawnConfig,
        theme: Theme,
        mux_tx: &mpsc::Sender<MuxEvent>,
        wakeup: Arc<dyn Fn() + Send + Sync>,
    ) -> io::Result<Pane> {
        // 1. Spawn PTY with the configured shell.
        let pty_config = PtyConfig {
            rows: config.rows,
            cols: config.cols,
            shell: config.shell.clone(),
            working_dir: config.cwd.clone(),
            env: config.env.clone(),
            shell_integration: config.shell_integration,
        };
        let mut pty = spawn_pty(&pty_config)?;

        // 2. Take handles before they're moved into the event loop.
        let reader = pty
            .take_reader()
            .ok_or_else(|| io::Error::other("PTY reader unavailable"))?;
        let writer = pty
            .take_writer()
            .ok_or_else(|| io::Error::other("PTY writer unavailable"))?;
        let control = pty
            .take_control()
            .ok_or_else(|| io::Error::other("PTY control unavailable"))?;

        // 3. Set up lock-free atomics for the mux event proxy.
        let wakeup_pending = Arc::new(AtomicBool::new(false));
        let grid_dirty = Arc::new(AtomicBool::new(false));
        let mode_cache = Arc::new(AtomicU32::new(0));

        // 4. Create the terminal state machine with a mux event proxy.
        let event_proxy = MuxEventProxy::new(
            pane_id,
            mux_tx.clone(),
            Arc::clone(&wakeup_pending),
            Arc::clone(&grid_dirty),
            wakeup,
        );
        let term = Term::new(
            usize::from(config.rows),
            usize::from(config.cols),
            config.scrollback,
            theme,
            event_proxy,
        );
        let terminal = Arc::new(FairMutex::new(term));

        // 5. Wire the message channel.
        let (tx, rx) = mpsc::channel();
        let notifier = PaneNotifier::new(writer, tx);

        // 6. Build and spawn the reader thread.
        let event_loop = PtyEventLoop::new(Arc::clone(&terminal), reader, rx);
        let reader_thread = event_loop.spawn()?;

        Ok(Pane::from_parts(PaneParts {
            id: pane_id,
            domain_id: self.id,
            terminal,
            notifier,
            pty_control: control,
            reader_thread,
            pty,
            grid_dirty,
            wakeup_pending,
            mode_cache,
        }))
    }
}
