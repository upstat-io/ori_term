use std::collections::HashSet;

use crate::id::{PaneId, TabId, WindowId};
use crate::layout::SplitDirection;

use super::{MuxTab, MuxWindow};

/// Build a `HashSet` of live pane IDs from a tab's current state.
fn live(tab: &MuxTab) -> HashSet<PaneId> {
    tab.all_panes().into_iter().collect()
}

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

    let live_panes = live(&tab);
    assert!(tab.undo_tree(&live_panes));
    assert_eq!(tab.all_panes(), vec![p1]);
}

#[test]
fn undo_empty_stack_returns_false() {
    let mut tab = MuxTab::new(TabId::from_raw(1), PaneId::from_raw(1));
    let live_panes = live(&tab);
    assert!(!tab.undo_tree(&live_panes));
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

    // Undo stack should be capped at 32. All panes referenced in undo
    // entries are still live (p1 is always present), so use a broad set.
    let all: HashSet<PaneId> = (1..42u64).map(PaneId::from_raw).collect();
    let mut count = 0;
    while tab.undo_tree(&all) {
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

    // Undo both. All referenced panes must be in the live set.
    let all: HashSet<PaneId> = [p1, p2, p3].into_iter().collect();
    assert!(tab.undo_tree(&all)); // back to 2 panes
    assert_eq!(tab.all_panes().len(), 2);
    assert!(tab.undo_tree(&all)); // back to 1 pane
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

// --- MuxWindow reorder_tab tests ---

#[test]
fn reorder_tab_basic_move() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    let t1 = TabId::from_raw(1);
    let t2 = TabId::from_raw(2);
    let t3 = TabId::from_raw(3);
    w.add_tab(t1);
    w.add_tab(t2);
    w.add_tab(t3);
    w.set_active_tab_idx(0); // t1 active

    assert!(w.reorder_tab(0, 2)); // move t1 to end
    assert_eq!(w.tabs(), &[t2, t3, t1]);
    assert_eq!(w.active_tab_idx(), 2); // active tracks t1
}

#[test]
fn reorder_tab_active_before_move() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    let t1 = TabId::from_raw(1);
    let t2 = TabId::from_raw(2);
    let t3 = TabId::from_raw(3);
    w.add_tab(t1);
    w.add_tab(t2);
    w.add_tab(t3);
    w.set_active_tab_idx(2); // t3 active

    // Move t1 (from=0) to position 2 (past active).
    assert!(w.reorder_tab(0, 2));
    assert_eq!(w.tabs(), &[t2, t3, t1]);
    // Active was at 2, from < active, to >= active → shift left.
    assert_eq!(w.active_tab_idx(), 1);
    assert_eq!(w.active_tab(), Some(t3));
}

#[test]
fn reorder_tab_active_after_move() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    let t1 = TabId::from_raw(1);
    let t2 = TabId::from_raw(2);
    let t3 = TabId::from_raw(3);
    w.add_tab(t1);
    w.add_tab(t2);
    w.add_tab(t3);
    w.set_active_tab_idx(0); // t1 active

    // Move t3 (from=2) to position 0 (before active).
    assert!(w.reorder_tab(2, 0));
    assert_eq!(w.tabs(), &[t3, t1, t2]);
    // Active was at 0, from > active, to <= active → shift right.
    assert_eq!(w.active_tab_idx(), 1);
    assert_eq!(w.active_tab(), Some(t1));
}

#[test]
fn reorder_tab_out_of_bounds() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    w.add_tab(TabId::from_raw(1));
    w.add_tab(TabId::from_raw(2));

    assert!(!w.reorder_tab(0, 5));
    assert!(!w.reorder_tab(5, 0));
}

#[test]
fn reorder_tab_noop_same_index() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    let t1 = TabId::from_raw(1);
    let t2 = TabId::from_raw(2);
    w.add_tab(t1);
    w.add_tab(t2);
    w.set_active_tab_idx(1);

    assert!(w.reorder_tab(1, 1));
    assert_eq!(w.tabs(), &[t1, t2]);
    assert_eq!(w.active_tab_idx(), 1);
}

/// set_active_tab_idx on an empty window is a no-op.
#[test]
fn set_active_tab_idx_on_empty_window() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    w.set_active_tab_idx(5);
    assert_eq!(w.active_tab_idx(), 0);
    assert!(w.active_tab().is_none());
}

// --- insert_tab_at tests ---

#[test]
fn insert_tab_at_start() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    w.add_tab(TabId::from_raw(1));
    w.add_tab(TabId::from_raw(2));
    w.set_active_tab_idx(0);

    w.insert_tab_at(0, TabId::from_raw(3));
    assert_eq!(
        w.tabs(),
        &[TabId::from_raw(3), TabId::from_raw(1), TabId::from_raw(2)]
    );
    // Active was at 0, insertion at 0 → shifts to 1.
    assert_eq!(w.active_tab(), Some(TabId::from_raw(1)));
    assert_eq!(w.active_tab_idx(), 1);
}

#[test]
fn insert_tab_at_middle() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    w.add_tab(TabId::from_raw(1));
    w.add_tab(TabId::from_raw(2));
    w.add_tab(TabId::from_raw(3));
    w.set_active_tab_idx(2);

    w.insert_tab_at(1, TabId::from_raw(4));
    assert_eq!(
        w.tabs(),
        &[
            TabId::from_raw(1),
            TabId::from_raw(4),
            TabId::from_raw(2),
            TabId::from_raw(3)
        ]
    );
    // Active was at 2, insertion at 1 (before) → shifts to 3.
    assert_eq!(w.active_tab(), Some(TabId::from_raw(3)));
    assert_eq!(w.active_tab_idx(), 3);
}

#[test]
fn insert_tab_at_end() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    w.add_tab(TabId::from_raw(1));
    w.add_tab(TabId::from_raw(2));
    w.set_active_tab_idx(0);

    // Inserting at len() appends.
    w.insert_tab_at(2, TabId::from_raw(3));
    assert_eq!(
        w.tabs(),
        &[TabId::from_raw(1), TabId::from_raw(2), TabId::from_raw(3)]
    );
    // Active was at 0, insertion at 2 (after) → no shift.
    assert_eq!(w.active_tab_idx(), 0);
}

#[test]
fn insert_tab_at_past_end_appends() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    w.add_tab(TabId::from_raw(1));

    w.insert_tab_at(100, TabId::from_raw(2));
    assert_eq!(w.tabs(), &[TabId::from_raw(1), TabId::from_raw(2)]);
}

#[test]
fn insert_tab_at_does_not_shift_active_when_after() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    w.add_tab(TabId::from_raw(1));
    w.add_tab(TabId::from_raw(2));
    w.add_tab(TabId::from_raw(3));
    w.set_active_tab_idx(0);

    // Insert after active — no shift needed.
    w.insert_tab_at(2, TabId::from_raw(4));
    assert_eq!(w.active_tab_idx(), 0);
    assert_eq!(w.active_tab(), Some(TabId::from_raw(1)));
}

#[test]
fn insert_tab_at_empty_window() {
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    w.insert_tab_at(0, TabId::from_raw(1));
    assert_eq!(w.tabs(), &[TabId::from_raw(1)]);
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

/// `set_floating()` replaces the entire floating layer.
#[test]
fn set_floating_replaces_layer() {
    let p1 = PaneId::from_raw(1);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    let fp = crate::layout::floating::FloatingPane {
        pane_id: PaneId::from_raw(2),
        rect: crate::layout::rect::Rect {
            x: 10.0,
            y: 20.0,
            width: 200.0,
            height: 100.0,
        },
        z_order: 1,
    };
    let layer = tab.floating().add(fp);
    tab.set_floating(layer);

    assert!(tab.floating().contains(PaneId::from_raw(2)));
    assert_eq!(tab.floating().panes().len(), 1);

    // Replace with empty layer.
    tab.set_floating(crate::layout::floating::FloatingLayer::new());
    assert!(tab.floating().is_empty());
}

/// `is_floating()` returns true only for floating pane IDs.
#[test]
fn is_floating_predicate() {
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    assert!(!tab.is_floating(p1));
    assert!(!tab.is_floating(p2));

    let fp = crate::layout::floating::FloatingPane {
        pane_id: p2,
        rect: crate::layout::rect::Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
        },
        z_order: 0,
    };
    tab.set_floating(tab.floating().add(fp));

    assert!(!tab.is_floating(p1));
    assert!(tab.is_floating(p2));
}

// --- Multi-step tab removal active index consistency ---

#[test]
fn multi_step_tab_removal_active_index_stays_valid() {
    // Create 4 tabs, then remove them in non-sequential order.
    // Active tab index must stay in-bounds at every step.
    let mut w = MuxWindow::new(WindowId::from_raw(1));
    let t1 = TabId::from_raw(1);
    let t2 = TabId::from_raw(2);
    let t3 = TabId::from_raw(3);
    let t4 = TabId::from_raw(4);

    w.add_tab(t1);
    w.add_tab(t2);
    w.add_tab(t3);
    w.add_tab(t4);
    w.set_active_tab_idx(2); // t3 is active

    // Remove t2 (before active) — active should shift left to keep t3.
    assert!(w.remove_tab(t2));
    assert_eq!(w.tabs(), &[t1, t3, t4]);
    assert_eq!(w.active_tab(), Some(t3));

    // Remove t1 (before active again) — active shifts left again.
    assert!(w.remove_tab(t1));
    assert_eq!(w.tabs(), &[t3, t4]);
    assert_eq!(w.active_tab(), Some(t3));

    // Active is now at index 0 (t3). Remove t4 (after active) — no shift.
    assert!(w.remove_tab(t4));
    assert_eq!(w.tabs(), &[t3]);
    assert_eq!(w.active_tab(), Some(t3));
    assert_eq!(w.active_tab_idx(), 0);

    // Remove the last tab.
    assert!(w.remove_tab(t3));
    assert!(w.tabs().is_empty());
    assert!(w.active_tab().is_none());
}

// --- MuxTab Send+Sync compile-time assertion ---

#[test]
fn mux_tab_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<MuxTab>();
    assert_send_sync::<MuxWindow>();
}

// --- Redo stack tests ---

#[test]
fn redo_stack_initialized_empty() {
    let p1 = PaneId::from_raw(1);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);
    let live_panes = live(&tab);
    assert!(!tab.redo_tree(&live_panes));
}

#[test]
fn set_tree_clears_redo_stack() {
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let p3 = PaneId::from_raw(3);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    // Split, then undo to populate redo stack.
    let tree2 = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree2);
    let all: HashSet<PaneId> = [p1, p2].into_iter().collect();
    assert!(tab.undo_tree(&all));

    // Now set_tree should clear redo.
    let tree3 = tab.tree().split_at(p1, SplitDirection::Horizontal, p3, 0.5);
    tab.set_tree(tree3);
    let all2: HashSet<PaneId> = [p1, p2, p3].into_iter().collect();
    assert!(!tab.redo_tree(&all2));
}

#[test]
fn undo_pushes_to_redo() {
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    let tree2 = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree2);
    let all: HashSet<PaneId> = [p1, p2].into_iter().collect();
    assert!(tab.undo_tree(&all));

    // Redo stack should now have the post-split tree.
    assert!(tab.redo_tree(&all));
    assert_eq!(tab.all_panes().len(), 2);
}

#[test]
fn redo_restores_undone_tree() {
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    let tree2 = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    let tree2_clone = tree2.clone();
    tab.set_tree(tree2);
    let all: HashSet<PaneId> = [p1, p2].into_iter().collect();

    assert!(tab.undo_tree(&all));
    assert_eq!(tab.all_panes(), vec![p1]);

    assert!(tab.redo_tree(&all));
    assert_eq!(*tab.tree(), tree2_clone);
}

#[test]
fn multiple_undo_then_redo_walks_forward() {
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let p3 = PaneId::from_raw(3);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    let tree2 = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree2);
    let tree3 = tab.tree().split_at(p1, SplitDirection::Horizontal, p3, 0.5);
    tab.set_tree(tree3);
    let all: HashSet<PaneId> = [p1, p2, p3].into_iter().collect();

    // Undo twice → single pane.
    assert!(tab.undo_tree(&all));
    assert!(tab.undo_tree(&all));
    assert_eq!(tab.all_panes(), vec![p1]);

    // Redo twice → back to 3 panes.
    assert!(tab.redo_tree(&all));
    assert_eq!(tab.all_panes().len(), 2);
    assert!(tab.redo_tree(&all));
    assert_eq!(tab.all_panes().len(), 3);
}

#[test]
fn new_mutation_after_undo_clears_redo() {
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let p3 = PaneId::from_raw(3);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    let tree2 = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree2);
    let all: HashSet<PaneId> = [p1, p2].into_iter().collect();
    assert!(tab.undo_tree(&all));

    // New mutation clears redo.
    let tree3 = tab.tree().split_at(p1, SplitDirection::Horizontal, p3, 0.5);
    tab.set_tree(tree3);
    let all2: HashSet<PaneId> = [p1, p2, p3].into_iter().collect();
    assert!(!tab.redo_tree(&all2));
}

#[test]
fn redo_empty_stack_returns_false() {
    let mut tab = MuxTab::new(TabId::from_raw(1), PaneId::from_raw(1));
    let live_panes = live(&tab);
    assert!(!tab.redo_tree(&live_panes));
}

#[test]
fn redo_stack_capped_at_32() {
    let p1 = PaneId::from_raw(1);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    // Push 40 mutations.
    for i in 2..42u64 {
        let p = PaneId::from_raw(i);
        let new_tree = tab.tree().split_at(p1, SplitDirection::Horizontal, p, 0.5);
        tab.set_tree(new_tree);
    }

    // Undo all 32 (capped) to fill the redo stack.
    let all: HashSet<PaneId> = (1..42u64).map(PaneId::from_raw).collect();
    let mut undo_count = 0;
    while tab.undo_tree(&all) {
        undo_count += 1;
    }
    assert_eq!(undo_count, 32);

    // Redo should also be capped at 32.
    let mut redo_count = 0;
    while tab.redo_tree(&all) {
        redo_count += 1;
    }
    assert_eq!(redo_count, 32);
}

#[test]
fn undo_skips_stale_pane_entry() {
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let p3 = PaneId::from_raw(3);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    // Split with p2.
    let tree2 = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree2);

    // Split with p3.
    let tree3 = tab.tree().split_at(p1, SplitDirection::Horizontal, p3, 0.5);
    tab.set_tree(tree3);

    // p2 was "closed" — not in live set.
    let live_panes: HashSet<PaneId> = [p1, p3].into_iter().collect();

    // First undo entry (tree with p1+p2) references stale p2, should be skipped.
    // Should fall through to the original tree (just p1), which is valid.
    assert!(tab.undo_tree(&live_panes));
    assert_eq!(tab.all_panes(), vec![p1]);
}

#[test]
fn redo_skips_stale_pane_entry() {
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let p3 = PaneId::from_raw(3);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    // Two splits.
    let tree2 = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree2);
    let tree3 = tab.tree().split_at(p1, SplitDirection::Horizontal, p3, 0.5);
    tab.set_tree(tree3);

    // Undo twice with all panes live.
    let all: HashSet<PaneId> = [p1, p2, p3].into_iter().collect();
    assert!(tab.undo_tree(&all));
    assert!(tab.undo_tree(&all));

    // Now p3 is "closed" — not in live set.
    // The redo stack has [tree with p1+p2, tree with p1+p2+p3].
    // The tree with p3 should be skipped, but tree with p1+p2 is valid.
    let live_panes: HashSet<PaneId> = [p1, p2].into_iter().collect();
    assert!(tab.redo_tree(&live_panes));
    assert_eq!(tab.all_panes().len(), 2);
    assert!(tab.tree().contains(p1));
    assert!(tab.tree().contains(p2));

    // No more valid redo entries.
    assert!(!tab.redo_tree(&live_panes));
}

/// `all_panes()` includes both tree and floating panes.
#[test]
fn all_panes_includes_floating_panes() {
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    assert_eq!(tab.all_panes(), vec![p1]);

    let fp = crate::layout::floating::FloatingPane {
        pane_id: p2,
        rect: crate::layout::rect::Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
        },
        z_order: 0,
    };
    tab.set_floating(tab.floating().add(fp));

    let all = tab.all_panes();
    assert_eq!(all.len(), 2);
    assert!(all.contains(&p1));
    assert!(all.contains(&p2));
}

// --- Undo/redo gap-analysis tests ---

/// Undo stack entries that reference multiple dead panes are all skipped.
#[test]
fn undo_skips_entries_with_multiple_dead_panes() {
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let p3 = PaneId::from_raw(3);
    let p4 = PaneId::from_raw(4);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    // Split with p2.
    let tree = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree);

    // Split with p3.
    let tree = tab.tree().split_at(p2, SplitDirection::Horizontal, p3, 0.5);
    tab.set_tree(tree);

    // Split with p4.
    let tree = tab.tree().split_at(p1, SplitDirection::Horizontal, p4, 0.5);
    tab.set_tree(tree);

    // Both p2 and p3 are "dead". Undo stack has:
    //   [0] leaf(p1)
    //   [1] p1+p2
    //   [2] p1+p2+p3
    // Only [0] is valid when live = {p1, p4}.
    let live: HashSet<PaneId> = [p1, p4].into_iter().collect();
    assert!(tab.undo_tree(&live));
    assert_eq!(tab.all_panes(), vec![p1]);

    // No more valid entries.
    assert!(!tab.undo_tree(&live));
}

/// Undo across mixed split directions restores correct tree shapes.
#[test]
fn undo_across_mixed_split_directions() {
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let p3 = PaneId::from_raw(3);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    // V-split: [p1 | p2].
    let tree_v = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree_v);

    // H-split: [[p1 / p3] | p2].
    let tree_vh = tab.tree().split_at(p1, SplitDirection::Horizontal, p3, 0.5);
    tab.set_tree(tree_vh);

    let all: HashSet<PaneId> = [p1, p2, p3].into_iter().collect();

    // Undo H-split → [p1 | p2].
    assert!(tab.undo_tree(&all));
    assert_eq!(tab.all_panes().len(), 2);
    assert!(tab.tree().contains(p1));
    assert!(tab.tree().contains(p2));
    assert!(!tab.tree().contains(p3));

    // Undo V-split → leaf(p1).
    assert!(tab.undo_tree(&all));
    assert_eq!(tab.all_panes(), vec![p1]);

    // Redo V-split → [p1 | p2].
    assert!(tab.redo_tree(&all));
    assert_eq!(tab.all_panes().len(), 2);
    assert!(tab.tree().contains(p1));
    assert!(tab.tree().contains(p2));
}

/// Redo entry referencing a dead pane is skipped.
#[test]
fn redo_skips_entry_after_pane_death() {
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let p3 = PaneId::from_raw(3);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    // Split with p2, then split with p3.
    let tree2 = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree2);
    let tree3 = tab.tree().split_at(p1, SplitDirection::Horizontal, p3, 0.5);
    tab.set_tree(tree3);

    // Undo back to single pane (all panes alive).
    let all: HashSet<PaneId> = [p1, p2, p3].into_iter().collect();
    assert!(tab.undo_tree(&all));
    assert!(tab.undo_tree(&all));
    assert_eq!(tab.all_panes(), vec![p1]);

    // Now p3 "dies". Redo stack has [tree with p1+p2, tree with p1+p2+p3].
    // The entry with p3 should be skipped; the one with p1+p2 is valid.
    let live: HashSet<PaneId> = [p1, p2].into_iter().collect();
    assert!(tab.redo_tree(&live));
    assert_eq!(tab.all_panes().len(), 2);
    assert!(tab.tree().contains(p1));
    assert!(tab.tree().contains(p2));

    // No more valid entries (the p3 entry was skipped).
    assert!(!tab.redo_tree(&live));
}

/// Floating layer is unaffected by split tree undo.
#[test]
fn floating_layer_unaffected_by_undo() {
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let fp_id = PaneId::from_raw(10);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    // Add a floating pane.
    let fp = crate::layout::floating::FloatingPane {
        pane_id: fp_id,
        rect: crate::layout::rect::Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
        },
        z_order: 0,
    };
    tab.set_floating(tab.floating().add(fp));

    // Split the tiled tree.
    let tree = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree);
    assert_eq!(tab.all_panes().len(), 3); // p1, p2, fp_id

    // Undo the split.
    let all: HashSet<PaneId> = [p1, p2, fp_id].into_iter().collect();
    assert!(tab.undo_tree(&all));

    // Tiled tree should be back to single pane.
    assert_eq!(tab.tree().panes(), vec![p1]);
    // Floating layer must be untouched.
    assert!(tab.floating().contains(fp_id));
    assert_eq!(tab.floating().panes().len(), 1);
    // all_panes includes both tiled + floating.
    assert_eq!(tab.all_panes().len(), 2);
}

/// When all undo entries reference dead panes, undo returns false.
#[test]
fn undo_with_all_panes_dead_returns_false() {
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let p3 = PaneId::from_raw(3);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    let tree = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree);
    let tree = tab.tree().split_at(p1, SplitDirection::Horizontal, p3, 0.5);
    tab.set_tree(tree);

    // All panes in undo entries are dead (only p99 is "live").
    let live: HashSet<PaneId> = [PaneId::from_raw(99)].into_iter().collect();
    assert!(!tab.undo_tree(&live));
}

/// When all redo entries reference dead panes, redo returns false.
#[test]
fn redo_with_all_panes_dead_returns_false() {
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let mut tab = MuxTab::new(TabId::from_raw(1), p1);

    let tree = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree);

    // Undo to populate redo stack (all panes alive).
    let all: HashSet<PaneId> = [p1, p2].into_iter().collect();
    assert!(tab.undo_tree(&all));

    // Now both p1 and p2 are "dead". Redo should fail.
    let empty: HashSet<PaneId> = [PaneId::from_raw(99)].into_iter().collect();
    assert!(!tab.redo_tree(&empty));
}
