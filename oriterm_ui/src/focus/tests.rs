//! Tests for focus management.

use crate::widget_id::WidgetId;

use super::FocusManager;

fn ids(n: usize) -> Vec<WidgetId> {
    (0..n).map(|_| WidgetId::next()).collect()
}

#[test]
fn new_manager_has_no_focus() {
    let mgr = FocusManager::new();
    assert_eq!(mgr.focused(), None);
    assert!(mgr.focus_order().is_empty());
}

#[test]
fn set_and_clear_focus() {
    let mut mgr = FocusManager::new();
    let id = WidgetId::next();

    mgr.set_focus(id);
    assert_eq!(mgr.focused(), Some(id));
    assert!(mgr.is_focused(id));

    mgr.clear_focus();
    assert_eq!(mgr.focused(), None);
    assert!(!mgr.is_focused(id));
}

#[test]
fn focus_next_wraps_around() {
    let mut mgr = FocusManager::new();
    let order = ids(3);
    mgr.set_focus_order(order.clone());

    // No focus → first.
    mgr.focus_next();
    assert_eq!(mgr.focused(), Some(order[0]));

    // Advance through.
    mgr.focus_next();
    assert_eq!(mgr.focused(), Some(order[1]));
    mgr.focus_next();
    assert_eq!(mgr.focused(), Some(order[2]));

    // Wrap around.
    mgr.focus_next();
    assert_eq!(mgr.focused(), Some(order[0]));
}

#[test]
fn focus_prev_wraps_around() {
    let mut mgr = FocusManager::new();
    let order = ids(3);
    mgr.set_focus_order(order.clone());

    // No focus → last.
    mgr.focus_prev();
    assert_eq!(mgr.focused(), Some(order[2]));

    // Go backward.
    mgr.focus_prev();
    assert_eq!(mgr.focused(), Some(order[1]));
    mgr.focus_prev();
    assert_eq!(mgr.focused(), Some(order[0]));

    // Wrap around.
    mgr.focus_prev();
    assert_eq!(mgr.focused(), Some(order[2]));
}

#[test]
fn focus_next_on_empty_order_is_noop() {
    let mut mgr = FocusManager::new();
    mgr.focus_next();
    assert_eq!(mgr.focused(), None);
}

#[test]
fn focus_prev_on_empty_order_is_noop() {
    let mut mgr = FocusManager::new();
    mgr.focus_prev();
    assert_eq!(mgr.focused(), None);
}

#[test]
fn set_focus_order_clears_stale_focus() {
    let mut mgr = FocusManager::new();
    let old_ids = ids(2);
    let new_ids = ids(2);

    mgr.set_focus_order(old_ids.clone());
    mgr.set_focus(old_ids[0]);

    // New order doesn't contain the old focused widget.
    mgr.set_focus_order(new_ids.clone());
    assert_eq!(mgr.focused(), None);
}

#[test]
fn set_focus_order_preserves_valid_focus() {
    let mut mgr = FocusManager::new();
    let order = ids(3);

    mgr.set_focus_order(order.clone());
    mgr.set_focus(order[1]);

    // Re-set same order — focus preserved.
    mgr.set_focus_order(order.clone());
    assert_eq!(mgr.focused(), Some(order[1]));
}

#[test]
fn single_widget_focus_cycles_to_itself() {
    let mut mgr = FocusManager::new();
    let order = ids(1);
    mgr.set_focus_order(order.clone());

    mgr.focus_next();
    assert_eq!(mgr.focused(), Some(order[0]));
    mgr.focus_next();
    assert_eq!(mgr.focused(), Some(order[0]));

    mgr.focus_prev();
    assert_eq!(mgr.focused(), Some(order[0]));
}

#[test]
fn focus_next_with_unknown_focused_resets_to_first() {
    let mut mgr = FocusManager::new();
    let order = ids(3);
    mgr.set_focus_order(order.clone());

    // Programmatically set focus to a widget not in the order.
    let rogue = WidgetId::next();
    mgr.set_focus(rogue);

    mgr.focus_next();
    assert_eq!(mgr.focused(), Some(order[0]));
}

#[test]
fn focus_prev_with_unknown_focused_resets_to_last() {
    let mut mgr = FocusManager::new();
    let order = ids(3);
    mgr.set_focus_order(order.clone());

    let rogue = WidgetId::next();
    mgr.set_focus(rogue);

    mgr.focus_prev();
    assert_eq!(mgr.focused(), Some(order[2]));
}
