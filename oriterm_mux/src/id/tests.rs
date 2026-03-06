//! Tests for mux identity types.

use std::collections::HashSet;

use super::{ClientId, DomainId, IdAllocator, MuxId, PaneId};

/// Compile-time trait bound checks: all ID types must be `Copy`, `Hash`, `Eq`.
#[test]
fn id_types_are_copy_hash_eq() {
    fn assert_id_traits<T: Copy + std::hash::Hash + Eq + std::fmt::Display + std::fmt::Debug>() {}
    assert_id_traits::<PaneId>();
    assert_id_traits::<DomainId>();
    assert_id_traits::<ClientId>();
}

#[test]
fn allocator_starts_at_one() {
    let mut alloc = IdAllocator::<PaneId>::new();
    assert_eq!(alloc.alloc(), PaneId::from_raw(1));
}

#[test]
fn allocator_produces_monotonically_increasing_values() {
    let mut alloc = IdAllocator::<PaneId>::new();
    let a = alloc.alloc();
    let b = alloc.alloc();
    let c = alloc.alloc();
    assert_eq!(a, PaneId::from_raw(1));
    assert_eq!(b, PaneId::from_raw(2));
    assert_eq!(c, PaneId::from_raw(3));
}

#[test]
fn allocator_values_are_unique() {
    let mut alloc = IdAllocator::<PaneId>::new();
    let mut seen = HashSet::new();
    for _ in 0..1000 {
        let id = alloc.alloc();
        assert!(seen.insert(id), "duplicate ID: {id}");
    }
}

#[test]
fn allocator_returns_correct_type() {
    let mut pane_alloc = IdAllocator::<PaneId>::new();
    let mut dom_alloc = IdAllocator::<DomainId>::new();
    let mut client_alloc = IdAllocator::<ClientId>::new();

    let pane: PaneId = pane_alloc.alloc();
    let dom: DomainId = dom_alloc.alloc();
    let client: ClientId = client_alloc.alloc();

    assert_eq!(pane.raw(), 1);
    assert_eq!(dom.raw(), 1);
    assert_eq!(client.raw(), 1);
}

#[test]
fn display_pane_id() {
    let id = PaneId::from_raw(42);
    assert_eq!(format!("{id}"), "Pane(42)");
}

#[test]
fn display_domain_id() {
    let id = DomainId::from_raw(5);
    assert_eq!(format!("{id}"), "Domain(5)");
}

#[test]
fn display_client_id() {
    let id = ClientId::from_raw(3);
    assert_eq!(format!("{id}"), "Client(3)");
}

#[test]
fn raw_round_trip() {
    assert_eq!(PaneId::from_raw(99).raw(), 99);
    assert_eq!(DomainId::from_raw(33).raw(), 33);
    assert_eq!(ClientId::from_raw(11).raw(), 11);
}

#[test]
fn mux_id_trait_round_trip() {
    fn check<T: MuxId + std::fmt::Debug + PartialEq>(val: u64) {
        let id = T::from_raw(val);
        assert_eq!(id.raw(), val);
    }
    check::<PaneId>(42);
    check::<DomainId>(5);
    check::<ClientId>(3);
}

/// Different ID types with the same raw value are distinct types.
#[test]
fn different_id_types_are_not_interchangeable() {
    let pane = PaneId::from_raw(1);
    let domain = DomainId::from_raw(1);
    let client = ClientId::from_raw(1);

    assert_eq!(pane, PaneId::from_raw(1));
    assert_eq!(domain, DomainId::from_raw(1));
    assert_eq!(client, ClientId::from_raw(1));
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
    let mut a = IdAllocator::<PaneId>::new();
    let mut b = IdAllocator::<PaneId>::default();
    assert_eq!(a.alloc(), b.alloc());
    assert_eq!(a.alloc(), b.alloc());
}
