use std::collections::HashSet;

use oriterm_mux::PaneId;

use super::Tab;
use crate::session::id::TabId;

fn pid(n: u64) -> PaneId {
    PaneId::from_raw(n)
}

fn tid(n: u64) -> TabId {
    TabId::from_raw(n)
}

#[test]
fn new_tab_has_single_pane() {
    let tab = Tab::new(tid(1), pid(10));
    assert_eq!(tab.id(), tid(1));
    assert_eq!(tab.active_pane(), pid(10));
    assert_eq!(tab.all_panes(), vec![pid(10)]);
    assert!(tab.zoomed_pane().is_none());
}

#[test]
fn set_active_pane() {
    let mut tab = Tab::new(tid(1), pid(10));
    tab.set_active_pane(pid(20));
    assert_eq!(tab.active_pane(), pid(20));
}

#[test]
fn zoom_state() {
    let mut tab = Tab::new(tid(1), pid(10));
    assert!(tab.zoomed_pane().is_none());

    tab.set_zoomed_pane(Some(pid(10)));
    assert_eq!(tab.zoomed_pane(), Some(pid(10)));

    tab.set_zoomed_pane(None);
    assert!(tab.zoomed_pane().is_none());
}

#[test]
fn set_tree_pushes_undo() {
    let mut tab = Tab::new(tid(1), pid(10));
    let original_tree = tab.tree().clone();

    // Replace tree with a new one.
    let new_tree = crate::session::split_tree::SplitTree::leaf(pid(20));
    tab.set_tree(new_tree.clone());

    assert_eq!(tab.tree().panes(), vec![pid(20)]);

    // Undo should restore the original.
    let live = HashSet::from([pid(10), pid(20)]);
    assert!(tab.undo_tree(&live));
    assert_eq!(tab.tree().panes(), original_tree.panes());
}

#[test]
fn undo_redo_cycle() {
    let mut tab = Tab::new(tid(1), pid(10));
    let live = HashSet::from([pid(10), pid(20)]);

    let tree_a = tab.tree().clone();
    let tree_b = crate::session::split_tree::SplitTree::leaf(pid(20));
    tab.set_tree(tree_b);

    // Undo: back to tree_a.
    assert!(tab.undo_tree(&live));
    assert_eq!(tab.tree().panes(), tree_a.panes());

    // Redo: forward to tree_b.
    assert!(tab.redo_tree(&live));
    assert_eq!(tab.tree().panes(), vec![pid(20)]);
}

#[test]
fn undo_skips_stale_entries() {
    let mut tab = Tab::new(tid(1), pid(10));

    // Push a tree referencing pid(20), then another referencing pid(10).
    let stale_tree = crate::session::split_tree::SplitTree::leaf(pid(20));
    tab.set_tree(stale_tree);
    let current = crate::session::split_tree::SplitTree::leaf(pid(10));
    tab.set_tree(current);

    // Only pid(10) is live — the stale tree (pid(20)) should be skipped.
    let live = HashSet::from([pid(10)]);
    assert!(tab.undo_tree(&live));
    // Should have skipped the stale pid(20) tree and found the original pid(10) tree.
    assert_eq!(tab.tree().panes(), vec![pid(10)]);
}

#[test]
fn undo_returns_false_when_empty() {
    let mut tab = Tab::new(tid(1), pid(10));
    let live = HashSet::from([pid(10)]);
    assert!(!tab.undo_tree(&live));
}

#[test]
fn redo_returns_false_when_empty() {
    let mut tab = Tab::new(tid(1), pid(10));
    let live = HashSet::from([pid(10)]);
    assert!(!tab.redo_tree(&live));
}

#[test]
fn replace_layout_does_not_push_undo() {
    let mut tab = Tab::new(tid(1), pid(10));
    let new_tree = crate::session::split_tree::SplitTree::leaf(pid(20));
    tab.replace_layout(new_tree);

    // Undo stack should be empty.
    let live = HashSet::from([pid(10), pid(20)]);
    assert!(!tab.undo_tree(&live));
}

#[test]
fn set_tree_clears_redo() {
    let mut tab = Tab::new(tid(1), pid(10));
    let live = HashSet::from([pid(10), pid(20), pid(30)]);

    let tree_b = crate::session::split_tree::SplitTree::leaf(pid(20));
    tab.set_tree(tree_b);

    // Undo to create a redo entry.
    assert!(tab.undo_tree(&live));

    // New mutation should clear redo.
    let tree_c = crate::session::split_tree::SplitTree::leaf(pid(30));
    tab.set_tree(tree_c);
    assert!(!tab.redo_tree(&live));
}

#[test]
fn floating_layer_initially_empty() {
    let tab = Tab::new(tid(1), pid(10));
    assert!(tab.floating().is_empty());
    assert!(!tab.is_floating(pid(10)));
}
