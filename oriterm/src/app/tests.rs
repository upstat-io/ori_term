//! Tests for app-level theme resolution and active pane resolution chain.

use oriterm_core::Theme;
use oriterm_ui::theme::UiTheme;

use oriterm_mux::session::{MuxTab, MuxWindow};
use oriterm_mux::{PaneId, SessionRegistry, TabId, WindowId};

use crate::config::{Config, ThemeOverride};

use super::resolve_ui_theme_with;

/// Mirror of `App::active_pane_id()` — same query chain, testable without App.
///
/// App::active_pane_id additionally requires `self.mux.as_ref()?` before
/// accessing the session. This helper tests the session resolution chain.
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
// These test the session query chain that `App::active_pane_id()` delegates to.
// App adds a None check for `self.mux` and `self.active_window` first, then
// performs the same chain: get_window → active_tab → get_tab → active_pane.

/// Build a session with one window, one tab, one pane.
fn session_with_one_pane() -> (SessionRegistry, WindowId, TabId, PaneId) {
    let mut session = SessionRegistry::new();
    let wid = WindowId::from_raw(1);
    let tid = TabId::from_raw(1);
    let pid = PaneId::from_raw(1);

    let mut win = MuxWindow::new(wid);
    win.add_tab(tid);
    session.add_window(win);
    session.add_tab(MuxTab::new(tid, pid));

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
    session.add_window(MuxWindow::new(wid));
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
    use oriterm_mux::layout::SplitDirection;

    let mut session = SessionRegistry::new();
    let wid = WindowId::from_raw(1);
    let tid = TabId::from_raw(1);
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);

    let mut win = MuxWindow::new(wid);
    win.add_tab(tid);
    session.add_window(win);

    let mut tab = MuxTab::new(tid, p1);
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
