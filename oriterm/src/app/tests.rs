//! Tests for app-level theme resolution, active pane resolution chain,
//! multi-window focus tracking, and focus event mode gating.

use oriterm_core::{TermMode, Theme};
use oriterm_ui::theme::UiTheme;

use oriterm_mux::PaneId;

use crate::config::{Config, ThemeOverride};
use crate::session::{SessionRegistry, Tab, TabId, Window, WindowId};

use super::resolve_ui_theme_with;

/// Mirror of `App::active_pane_id()` — same query chain, testable without App.
///
/// App::active_pane_id reads from `self.session` (local session registry).
/// This helper tests the session resolution chain.
fn resolve_active_pane(
    session: &SessionRegistry,
    active_window: Option<WindowId>,
) -> Option<PaneId> {
    let win_id = active_window?;
    let win = session.get_window(win_id)?;
    let tab_id = win.active_tab()?;
    let tab = session.get_tab(tab_id)?;
    Some(tab.active_pane())
}

// ── resolve_ui_theme_with: ThemeOverride → UiTheme mapping ──

#[test]
fn resolve_dark_override_ignores_system() {
    let mut config = Config::default();
    config.colors.theme = ThemeOverride::Dark;
    // System says Light, but override says Dark → dark theme.
    assert_eq!(
        resolve_ui_theme_with(&config, Theme::Light),
        UiTheme::dark()
    );
}

#[test]
fn resolve_light_override_ignores_system() {
    let mut config = Config::default();
    config.colors.theme = ThemeOverride::Light;
    // System says Dark, but override says Light → light theme.
    assert_eq!(
        resolve_ui_theme_with(&config, Theme::Dark),
        UiTheme::light()
    );
}

#[test]
fn resolve_auto_delegates_to_system_light() {
    let mut config = Config::default();
    config.colors.theme = ThemeOverride::Auto;
    assert_eq!(
        resolve_ui_theme_with(&config, Theme::Light),
        UiTheme::light()
    );
}

#[test]
fn resolve_auto_delegates_to_system_dark() {
    let mut config = Config::default();
    config.colors.theme = ThemeOverride::Auto;
    assert_eq!(resolve_ui_theme_with(&config, Theme::Dark), UiTheme::dark());
}

#[test]
fn resolve_auto_unknown_falls_back_to_dark() {
    let mut config = Config::default();
    config.colors.theme = ThemeOverride::Auto;
    assert_eq!(
        resolve_ui_theme_with(&config, Theme::Unknown),
        UiTheme::dark(),
    );
}

// -- active_pane_id resolution chain --
//
// These test the session query chain that `App::active_pane_id()` uses.
// App reads from `self.session` (local session registry): get_window →
// active_tab → get_tab → active_pane.

/// Build a session with one window, one tab, one pane.
fn session_with_one_pane() -> (SessionRegistry, WindowId, TabId, PaneId) {
    let mut session = SessionRegistry::new();
    let wid = WindowId::from_raw(1);
    let tid = TabId::from_raw(1);
    let pid = PaneId::from_raw(1);

    let mut win = Window::new(wid);
    win.add_tab(tid);
    session.add_window(win);
    session.add_tab(Tab::new(tid, pid));

    (session, wid, tid, pid)
}

#[test]
fn active_pane_resolve_none_when_no_active_window() {
    let (session, _wid, _tid, _pid) = session_with_one_pane();
    // active_window is None → should return None immediately.
    assert_eq!(resolve_active_pane(&session, None), None);
}

#[test]
fn active_pane_resolve_none_for_stale_window_id() {
    let (session, _wid, _tid, _pid) = session_with_one_pane();
    // Window ID that doesn't exist in the session.
    let stale = WindowId::from_raw(999);
    assert_eq!(resolve_active_pane(&session, Some(stale)), None);
}

#[test]
fn active_pane_resolve_none_for_empty_window() {
    let mut session = SessionRegistry::new();
    let wid = WindowId::from_raw(1);
    // Window exists but has no tabs.
    session.add_window(Window::new(wid));
    assert_eq!(resolve_active_pane(&session, Some(wid)), None);
}

#[test]
fn active_pane_resolve_happy_path() {
    let (session, wid, _tid, pid) = session_with_one_pane();
    assert_eq!(resolve_active_pane(&session, Some(wid)), Some(pid));
}

#[test]
fn active_pane_resolve_after_close_returns_reassigned() {
    // Two panes in one tab. Close the active pane → active should shift.
    use crate::session::SplitDirection;

    let mut session = SessionRegistry::new();
    let wid = WindowId::from_raw(1);
    let tid = TabId::from_raw(1);
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);

    let mut win = Window::new(wid);
    win.add_tab(tid);
    session.add_window(win);

    let mut tab = Tab::new(tid, p1);
    let tree = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree);
    session.add_tab(tab);

    // Active is p1. Simulate close_pane(p1): remove from tree, reassign active.
    let tab = session.get_tab_mut(tid).unwrap();
    let new_tree = tab.tree().remove(p1).expect("p2 remains");
    tab.set_tree(new_tree);
    tab.set_active_pane(p2);

    assert_eq!(resolve_active_pane(&session, Some(wid)), Some(p2));
}

#[test]
fn active_pane_resolve_none_after_all_closed() {
    let (mut session, wid, tid, _pid) = session_with_one_pane();

    // Remove the tab entirely (simulates last pane closed → tab removed).
    session.remove_tab(tid);
    session.get_window_mut(wid).unwrap().remove_tab(tid);

    // Window still exists but has no tabs → None.
    assert_eq!(resolve_active_pane(&session, Some(wid)), None);
}

// -- Focus event mode gating --
//
// `send_focus_event` checks `TermMode::FOCUS_IN_OUT` via a bitmask on the
// lock-free mode cache. These tests verify the bit pattern matches expectations.

#[test]
fn focus_in_out_mode_bit_pattern() {
    // FOCUS_IN_OUT is bit 12 (1 << 12 = 0x1000).
    let bits = TermMode::FOCUS_IN_OUT.bits();
    assert_eq!(bits, 0x1000);
    // Mode cache with FOCUS_IN_OUT set should pass the mask check.
    assert_ne!(bits & TermMode::FOCUS_IN_OUT.bits(), 0);
}

#[test]
fn focus_in_out_not_set_by_default() {
    // Empty mode should not have FOCUS_IN_OUT.
    let empty = TermMode::empty().bits();
    assert_eq!(empty & TermMode::FOCUS_IN_OUT.bits(), 0);
}

#[test]
fn focus_in_out_combined_with_other_modes() {
    // FOCUS_IN_OUT combined with other modes still passes the check.
    let combined = TermMode::FOCUS_IN_OUT | TermMode::BRACKETED_PASTE;
    assert_ne!(combined.bits() & TermMode::FOCUS_IN_OUT.bits(), 0);
}

// -- Multi-window active_window tracking --
//
// When focus moves between windows, `active_window` updates to track which
// session window corresponds to the focused OS window. These tests verify the
// session model supports distinct per-window pane resolution.

#[test]
fn multi_window_focus_switch_resolves_different_panes() {
    let mut session = SessionRegistry::new();

    // Window 1: tab with pane A.
    let w1 = WindowId::from_raw(1);
    let t1 = TabId::from_raw(1);
    let pa = PaneId::from_raw(1);
    let mut win1 = Window::new(w1);
    win1.add_tab(t1);
    session.add_window(win1);
    session.add_tab(Tab::new(t1, pa));

    // Window 2: tab with pane B.
    let w2 = WindowId::from_raw(2);
    let t2 = TabId::from_raw(2);
    let pb = PaneId::from_raw(2);
    let mut win2 = Window::new(w2);
    win2.add_tab(t2);
    session.add_window(win2);
    session.add_tab(Tab::new(t2, pb));

    // Focus window 1 → active pane is A.
    assert_eq!(resolve_active_pane(&session, Some(w1)), Some(pa));
    // Focus window 2 → active pane is B.
    assert_eq!(resolve_active_pane(&session, Some(w2)), Some(pb));
    // Switch back to window 1 → still pane A.
    assert_eq!(resolve_active_pane(&session, Some(w1)), Some(pa));
}

#[test]
fn multi_window_stale_window_returns_none() {
    let mut session = SessionRegistry::new();

    let w1 = WindowId::from_raw(1);
    let t1 = TabId::from_raw(1);
    let pa = PaneId::from_raw(1);
    let mut win1 = Window::new(w1);
    win1.add_tab(t1);
    session.add_window(win1);
    session.add_tab(Tab::new(t1, pa));

    // Focus a window that doesn't exist → None.
    let stale = WindowId::from_raw(42);
    assert_eq!(resolve_active_pane(&session, Some(stale)), None);
}

// -- Window lifecycle: close and focus transfer --
//
// These test the session-level logic that `close_window` and
// `handle_mux_window_closed` rely on: removing a window from the
// session model, verifying remaining windows resolve correctly,
// and confirming focus can transfer to a surviving window.

/// Build a two-window session: each window has one tab with one pane.
fn two_window_session() -> (SessionRegistry, WindowId, WindowId, PaneId, PaneId) {
    let mut session = SessionRegistry::new();

    let w1 = WindowId::from_raw(1);
    let t1 = TabId::from_raw(1);
    let p1 = PaneId::from_raw(1);
    let mut win1 = Window::new(w1);
    win1.add_tab(t1);
    session.add_window(win1);
    session.add_tab(Tab::new(t1, p1));

    let w2 = WindowId::from_raw(2);
    let t2 = TabId::from_raw(2);
    let p2 = PaneId::from_raw(2);
    let mut win2 = Window::new(w2);
    win2.add_tab(t2);
    session.add_window(win2);
    session.add_tab(Tab::new(t2, p2));

    (session, w1, w2, p1, p2)
}

#[test]
fn close_window_focus_transfers_to_remaining() {
    let (mut session, w1, w2, _p1, p2) = two_window_session();

    // Simulate closing window 1: remove it from the session.
    session.remove_window(w1);

    // Focus should transfer to window 2.
    assert_eq!(session.window_count(), 1);
    assert_eq!(resolve_active_pane(&session, Some(w2)), Some(p2));
    // Old window no longer resolves.
    assert_eq!(resolve_active_pane(&session, Some(w1)), None);
}

#[test]
fn close_window_cleans_up_tabs() {
    let (mut session, w1, w2, _p1, p2) = two_window_session();

    // Get tab IDs before close.
    let t1 = session.get_window(w1).unwrap().tabs()[0];
    let t2 = session.get_window(w2).unwrap().tabs()[0];

    // Close window 1 — remove its tabs too (mimics mux.close_window).
    session.remove_tab(t1);
    session.remove_window(w1);

    // Window 1's tab is gone, window 2's tab still exists.
    assert!(session.get_tab(t1).is_none());
    assert!(session.get_tab(t2).is_some());
    assert_eq!(session.tab_count(), 1);

    // Window 2 still resolves normally.
    assert_eq!(resolve_active_pane(&session, Some(w2)), Some(p2));
}

#[test]
fn close_all_windows_leaves_empty_session() {
    let (mut session, w1, w2, _p1, _p2) = two_window_session();

    // Close both windows.
    let t1 = session.get_window(w1).unwrap().tabs()[0];
    let t2 = session.get_window(w2).unwrap().tabs()[0];
    session.remove_tab(t1);
    session.remove_window(w1);
    session.remove_tab(t2);
    session.remove_window(w2);

    assert_eq!(session.window_count(), 0);
    assert_eq!(session.tab_count(), 0);
    assert_eq!(resolve_active_pane(&session, Some(w1)), None);
    assert_eq!(resolve_active_pane(&session, Some(w2)), None);
}

#[test]
fn multi_window_close_preserves_other_window_tabs() {
    // Three windows. Close the middle one. Windows 1 and 3 unaffected.
    let mut session = SessionRegistry::new();

    let ids: Vec<_> = (1..=3)
        .map(|i| {
            let w = WindowId::from_raw(i);
            let t = TabId::from_raw(i);
            let p = PaneId::from_raw(i);
            let mut win = Window::new(w);
            win.add_tab(t);
            session.add_window(win);
            session.add_tab(Tab::new(t, p));
            (w, t, p)
        })
        .collect();

    // Close window 2.
    session.remove_tab(ids[1].1);
    session.remove_window(ids[1].0);

    assert_eq!(session.window_count(), 2);
    assert_eq!(
        resolve_active_pane(&session, Some(ids[0].0)),
        Some(ids[0].2)
    );
    assert_eq!(
        resolve_active_pane(&session, Some(ids[2].0)),
        Some(ids[2].2)
    );
    assert_eq!(resolve_active_pane(&session, Some(ids[1].0)), None);
}
