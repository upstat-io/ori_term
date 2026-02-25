use std::collections::HashSet;

use super::{IdAllocator, PaneId, SessionId, TabId, WindowId};

/// Compile-time trait bound checks: all ID types must be `Copy`, `Hash`, `Eq`.
#[test]
fn id_types_are_copy_hash_eq() {
    fn assert_id_traits<T: Copy + std::hash::Hash + Eq + std::fmt::Display + std::fmt::Debug>() {}
    assert_id_traits::<PaneId>();
    assert_id_traits::<TabId>();
    assert_id_traits::<WindowId>();
    assert_id_traits::<SessionId>();
}

#[test]
fn allocator_starts_at_one() {
    let mut alloc = IdAllocator::new();
    assert_eq!(alloc.alloc(), 1);
}

#[test]
fn allocator_produces_monotonically_increasing_values() {
    let mut alloc = IdAllocator::new();
    let a = alloc.alloc();
    let b = alloc.alloc();
    let c = alloc.alloc();
    assert_eq!(a, 1);
    assert_eq!(b, 2);
    assert_eq!(c, 3);
    assert!(a < b);
    assert!(b < c);
}

#[test]
fn allocator_values_are_unique() {
    let mut alloc = IdAllocator::new();
    let mut seen = HashSet::new();
    for _ in 0..1000 {
        let id = alloc.alloc();
        assert!(seen.insert(id), "duplicate ID: {id}");
    }
}

#[test]
fn display_pane_id() {
    let id = PaneId::from_raw(42);
    assert_eq!(format!("{id}"), "Pane(42)");
}

#[test]
fn display_tab_id() {
    let id = TabId::from_raw(7);
    assert_eq!(format!("{id}"), "Tab(7)");
}

#[test]
fn display_window_id() {
    let id = WindowId::from_raw(3);
    assert_eq!(format!("{id}"), "Window(3)");
}

#[test]
fn display_session_id() {
    let id = SessionId::from_raw(1);
    assert_eq!(format!("{id}"), "Session(1)");
}

#[test]
fn raw_round_trip() {
    assert_eq!(PaneId::from_raw(99).raw(), 99);
    assert_eq!(TabId::from_raw(50).raw(), 50);
    assert_eq!(WindowId::from_raw(11).raw(), 11);
    assert_eq!(SessionId::from_raw(77).raw(), 77);
}

/// Different ID types with the same raw value must not be equal.
/// This is enforced by the type system — they are distinct types.
/// This test documents the intent rather than testing runtime behavior.
#[test]
fn different_id_types_are_not_interchangeable() {
    let pane = PaneId::from_raw(1);
    let tab = TabId::from_raw(1);
    let window = WindowId::from_raw(1);
    let session = SessionId::from_raw(1);

    // These are different types — they cannot be compared with `==`.
    // The below assertions verify they can each be used in type-specific
    // contexts without confusion.
    assert_eq!(pane, PaneId::from_raw(1));
    assert_eq!(tab, TabId::from_raw(1));
    assert_eq!(window, WindowId::from_raw(1));
    assert_eq!(session, SessionId::from_raw(1));
}

#[test]
fn ids_work_as_hash_keys() {
    let mut pane_set = HashSet::new();
    pane_set.insert(PaneId::from_raw(1));
    pane_set.insert(PaneId::from_raw(2));
    pane_set.insert(PaneId::from_raw(1)); // duplicate

    assert_eq!(pane_set.len(), 2);
    assert!(pane_set.contains(&PaneId::from_raw(1)));
    assert!(pane_set.contains(&PaneId::from_raw(2)));
    assert!(!pane_set.contains(&PaneId::from_raw(3)));
}

#[test]
fn allocator_default_same_as_new() {
    let mut a = IdAllocator::new();
    let mut b = IdAllocator::default();
    assert_eq!(a.alloc(), b.alloc());
    assert_eq!(a.alloc(), b.alloc());
}
