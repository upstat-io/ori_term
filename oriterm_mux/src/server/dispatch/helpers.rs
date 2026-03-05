//! Shared dispatch helpers — theme parsing, pane cleanup, subscription removal.

use std::collections::HashMap;

use oriterm_core::Theme;

use crate::PaneId;
use crate::id::ClientId;
use crate::pane::Pane;

/// Drop a pane on a background thread to avoid blocking the server event loop.
///
/// `Pane::drop` signals shutdown and kills the child process, but the field
/// destructors (especially `PtyHandle.child`) can block on Windows/ConPTY
/// cleanup. Spawning a thread ensures the server responds to RPCs promptly.
pub(in crate::server) fn drop_pane_background(pane: Option<Pane>) {
    if let Some(pane) = pane {
        std::thread::spawn(move || drop(pane));
    }
}

/// Parse a wire theme string into a [`Theme`].
///
/// `None` or unrecognized strings default to [`Theme::Dark`].
pub(in crate::server) fn parse_theme(s: Option<&str>) -> Theme {
    match s {
        Some("light") => Theme::Light,
        _ => Theme::Dark,
    }
}

/// Remove all pane subscriptions from the global subscriptions map for a
/// disconnecting client.
pub(in crate::server) fn remove_client_subscriptions(
    subscriptions: &mut HashMap<PaneId, Vec<ClientId>>,
    client_id: ClientId,
    subscribed_panes: &std::collections::HashSet<PaneId>,
) {
    for pane_id in subscribed_panes {
        if let Some(subs) = subscriptions.get_mut(pane_id) {
            subs.retain(|&c| c != client_id);
            if subs.is_empty() {
                subscriptions.remove(pane_id);
            }
        }
    }
}
