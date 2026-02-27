use crate::id::{PaneId, TabId, WindowId};
use crate::layout::SplitDirection;

use super::{MuxTab, MuxWindow};

// --- MuxTab tests ---

#[test]
fn new_tab_has_single_pane() {
    let tab = MuxTab::new(TabId::from_raw(1), PaneId::from_raw(10));
    assert_eq!(tab.id(), TabId::from_raw(1));
    assert_eq!(tab.active_pane(), PaneId::from_raw(10));
    assert_eq!(tab.all_panes(), vec![PaneId::from_raw(10)]);
}

#[test]
fn set_tree_pushes_undo() {
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    let new_tree = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(new_tree);

    assert_eq!(tab.all_panes().len(), 2);
    assert!(tab.tree().contains(p1));
    assert!(tab.tree().contains(p2));
}

#[test]
fn undo_restores_previous_tree() {
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    let new_tree = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(new_tree);
    assert_eq!(tab.all_panes().len(), 2);

    assert!(tab.undo_tree());
    assert_eq!(tab.all_panes(), vec![p1]);
}

#[test]
fn undo_empty_stack_returns_false() {
    let mut tab = MuxTab::new(TabId::from_raw(1), PaneId::from_raw(1));
    assert!(!tab.undo_tree());
}

#[test]
fn undo_stack_capped_at_32() {
    let p1 = PaneId::from_raw(1);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    // Push 40 tree mutations.
    for i in 2..42u64 {
        let p = PaneId::from_raw(i);
        let new_tree = tab.tree().split_at(p1, SplitDirection::Horizontal, p, 0.5);
        tab.set_tree(new_tree);
    }

    // Undo stack should be capped at 32.
    let mut count = 0;
    while tab.undo_tree() {
        count += 1;
    }
    assert_eq!(count, 32);
}

#[test]
fn set_active_pane() {
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);
    tab.set_active_pane(p2);
    assert_eq!(tab.active_pane(), p2);
}

// --- MuxWindow tests ---

#[test]
fn new_window_is_empty() {
    let w = MuxWindow::new(WindowId::from_raw(1));
    assert_eq!(w.id(), WindowId::from_raw(1));
    assert!(w.tabs().is_empty());
    assert_eq!(w.active_tab_idx(), 0);
    assert!(w.active_tab().is_none());
}

#[test]
fn add_tab_appends() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    w.add_tab(TabId::from_raw(10));
    w.add_tab(TabId::from_raw(20));
    assert_eq!(w.tabs(), &[TabId::from_raw(10), TabId::from_raw(20)]);
}

#[test]
fn active_tab_after_add() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    w.add_tab(TabId::from_raw(10));
    assert_eq!(w.active_tab(), Some(TabId::from_raw(10)));
}

#[test]
fn remove_tab_adjusts_active_before() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    w.add_tab(TabId::from_raw(1));
    w.add_tab(TabId::from_raw(2));
    w.add_tab(TabId::from_raw(3));
    w.set_active_tab_idx(2); // tab 3 is active

    // Remove tab before active — active should shift left.
    assert!(w.remove_tab(TabId::from_raw(1)));
    assert_eq!(w.active_tab_idx(), 1);
    assert_eq!(w.active_tab(), Some(TabId::from_raw(3)));
}

#[test]
fn remove_active_tab_clamps() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    w.add_tab(TabId::from_raw(1));
    w.add_tab(TabId::from_raw(2));
    w.set_active_tab_idx(1);

    // Remove the active (last) tab — index should clamp to new last.
    assert!(w.remove_tab(TabId::from_raw(2)));
    assert_eq!(w.active_tab_idx(), 0);
    assert_eq!(w.active_tab(), Some(TabId::from_raw(1)));
}

#[test]
fn remove_nonexistent_tab_returns_false() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    w.add_tab(TabId::from_raw(1));
    assert!(!w.remove_tab(TabId::from_raw(99)));
}

#[test]
fn remove_last_tab_resets() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    w.add_tab(TabId::from_raw(1));
    assert!(w.remove_tab(TabId::from_raw(1)));
    assert!(w.tabs().is_empty());
    assert_eq!(w.active_tab_idx(), 0);
    assert!(w.active_tab().is_none());
}

#[test]
fn set_active_tab_idx_clamps() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    w.add_tab(TabId::from_raw(1));
    w.add_tab(TabId::from_raw(2));
    w.set_active_tab_idx(999);
    assert_eq!(w.active_tab_idx(), 1);
}

// --- Gap analysis tests ---

/// Removing a tab that is after the active tab leaves active_tab_idx unchanged.
#[test]
fn remove_tab_after_active_is_noop() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    w.add_tab(TabId::from_raw(1));
    w.add_tab(TabId::from_raw(2));
    w.add_tab(TabId::from_raw(3));
    w.set_active_tab_idx(0); // tab 1 is active

    assert!(w.remove_tab(TabId::from_raw(3))); // remove tab after active
    assert_eq!(w.active_tab_idx(), 0);
    assert_eq!(w.active_tab(), Some(TabId::from_raw(1)));
    assert_eq!(w.tabs(), &[TabId::from_raw(1), TabId::from_raw(2)]);
}

/// Removing the active tab when it's in the middle clamps correctly.
#[test]
fn remove_active_tab_in_middle() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    w.add_tab(TabId::from_raw(1));
    w.add_tab(TabId::from_raw(2));
    w.add_tab(TabId::from_raw(3));
    w.set_active_tab_idx(1); // tab 2 is active

    // Remove the active tab (middle). active_tab_idx == pos, and
    // since tabs.len() was 3 (now 2), idx 1 is still valid.
    assert!(w.remove_tab(TabId::from_raw(2)));
    assert_eq!(w.tabs(), &[TabId::from_raw(1), TabId::from_raw(3)]);
    // Active index stays at 1 (now pointing to tab 3).
    assert_eq!(w.active_tab_idx(), 1);
    assert_eq!(w.active_tab(), Some(TabId::from_raw(3)));
}

/// Multiple undos followed by a new split works correctly.
#[test]
fn multiple_undo_then_re_split() {
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let p3 = PaneId::from_raw(3);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    // Split twice.
    let tree2 = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree2);
    let tree3 = tab.tree().split_at(p1, SplitDirection::Horizontal, p3, 0.5);
    tab.set_tree(tree3);
    assert_eq!(tab.all_panes().len(), 3);

    // Undo both.
    assert!(tab.undo_tree()); // back to 2 panes
    assert_eq!(tab.all_panes().len(), 2);
    assert!(tab.undo_tree()); // back to 1 pane
    assert_eq!(tab.all_panes(), vec![p1]);

    // Re-split: new tree on a clean undo stack should work fine.
    let p4 = PaneId::from_raw(4);
    let tree4 = tab.tree().split_at(p1, SplitDirection::Vertical, p4, 0.3);
    tab.set_tree(tree4);
    assert_eq!(tab.all_panes().len(), 2);
    assert!(tab.tree().contains(p4));
}

/// all_panes returns the correct set after multiple splits.
#[test]
fn all_panes_after_multiple_splits() {
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let p3 = PaneId::from_raw(3);
    let p4 = PaneId::from_raw(4);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    let tree = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree);
    let tree = tab.tree().split_at(p2, SplitDirection::Horizontal, p3, 0.5);
    tab.set_tree(tree);
    let tree = tab.tree().split_at(p1, SplitDirection::Horizontal, p4, 0.5);
    tab.set_tree(tree);

    let mut panes = tab.all_panes();
    panes.sort_by_key(|p| p.raw());
    assert_eq!(panes, vec![p1, p2, p3, p4]);
}

/// set_active_tab_idx on an empty window is a no-op.
#[test]
fn set_active_tab_idx_on_empty_window() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    w.set_active_tab_idx(5);
    assert_eq!(w.active_tab_idx(), 0);
    assert!(w.active_tab().is_none());
}

// --- Zoom state tests ---

#[test]
fn zoomed_pane_default_none() {
    let tab = MuxTab::new(TabId::from_raw(1), PaneId::from_raw(1));
    assert_eq!(tab.zoomed_pane(), None);
}

#[test]
fn set_zoomed_pane_roundtrip() {
    let p1 = PaneId::from_raw(1);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);
    tab.set_zoomed_pane(Some(p1));
    assert_eq!(tab.zoomed_pane(), Some(p1));
}

#[test]
fn zoomed_pane_cleared_on_none() {
    let p1 = PaneId::from_raw(1);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);
    tab.set_zoomed_pane(Some(p1));
    tab.set_zoomed_pane(None);
    assert_eq!(tab.zoomed_pane(), None);
}

/// Floating layer is accessible via MuxTab.
#[test]
fn floating_layer_accessible() {
    let p1 = PaneId::from_raw(1);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    assert!(tab.floating().is_empty());

    // FloatingLayer is immutable — add returns a new layer.
    let pane = crate::layout::floating::FloatingPane {
        pane_id: PaneId::from_raw(2),
        rect: crate::layout::rect::Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
        },
        z_order: 0,
    };
    let new_layer = tab.floating().add(pane);
    *tab.floating_mut() = new_layer;

    assert_eq!(tab.floating().panes().len(), 1);
    assert!(tab.floating().contains(PaneId::from_raw(2)));
}
