//! Tests for [`PaneRegistry`].

use crate::id::{DomainId, PaneId};

use super::{PaneEntry, PaneRegistry};

fn entry(pane: u64, domain: u64) -> PaneEntry {
    PaneEntry {
        pane: PaneId::from_raw(pane),
        domain: DomainId::from_raw(domain),
    }
}

#[test]
fn empty_registry() {
    let reg = PaneRegistry::new();
    assert!(reg.is_empty());
    assert_eq!(reg.len(), 0);
    assert!(reg.get(PaneId::from_raw(1)).is_none());
}

#[test]
fn register_and_get() {
    let mut reg = PaneRegistry::new();
    reg.register(entry(1, 1));

    let e = reg.get(PaneId::from_raw(1)).unwrap();
    assert_eq!(e.pane, PaneId::from_raw(1));
    assert_eq!(e.domain, DomainId::from_raw(1));
}

#[test]
fn unregister_removes_entry() {
    let mut reg = PaneRegistry::new();
    reg.register(entry(1, 1));
    assert_eq!(reg.len(), 1);

    let removed = reg.unregister(PaneId::from_raw(1));
    assert!(removed.is_some());
    assert!(reg.is_empty());
}

#[test]
fn unregister_nonexistent_returns_none() {
    let mut reg = PaneRegistry::new();
    assert!(reg.unregister(PaneId::from_raw(99)).is_none());
}

/// Registering the same pane ID twice overwrites the first entry.
#[test]
fn register_overwrites_existing_entry() {
    let mut reg = PaneRegistry::new();
    reg.register(entry(1, 1));
    reg.register(entry(1, 2));

    assert_eq!(reg.len(), 1);
    let e = reg.get(PaneId::from_raw(1)).unwrap();
    assert_eq!(e.domain, DomainId::from_raw(2));
}

/// Panes from multiple domains coexist and are distinguishable.
#[test]
fn multiple_domains_in_registry() {
    let mut reg = PaneRegistry::new();
    reg.register(entry(1, 1));
    reg.register(entry(2, 2));
    reg.register(entry(3, 1));

    assert_eq!(reg.len(), 3);
    assert_eq!(
        reg.get(PaneId::from_raw(1)).unwrap().domain,
        DomainId::from_raw(1)
    );
    assert_eq!(
        reg.get(PaneId::from_raw(2)).unwrap().domain,
        DomainId::from_raw(2)
    );
}

/// After unregistering a pane, queries no longer return it.
#[test]
fn consistent_after_unregister() {
    let mut reg = PaneRegistry::new();
    reg.register(entry(1, 1));
    reg.register(entry(2, 1));
    reg.register(entry(3, 1));

    reg.unregister(PaneId::from_raw(2));

    assert_eq!(reg.len(), 2);
    assert!(reg.get(PaneId::from_raw(2)).is_none());
    assert!(reg.get(PaneId::from_raw(1)).is_some());
    assert!(reg.get(PaneId::from_raw(3)).is_some());
}

/// Registry handles 1000+ entries without issue.
#[test]
fn large_registry_stress() {
    let mut reg = PaneRegistry::new();
    for i in 0..1000u64 {
        reg.register(PaneEntry {
            pane: PaneId::from_raw(i),
            domain: DomainId::from_raw(1),
        });
    }
    assert_eq!(reg.len(), 1000);

    // Unregister all even panes.
    for i in (0..1000u64).step_by(2) {
        reg.unregister(PaneId::from_raw(i));
    }
    assert_eq!(reg.len(), 500);

    // Odd panes should still be present.
    assert!(reg.get(PaneId::from_raw(1)).is_some());
    assert!(reg.get(PaneId::from_raw(999)).is_some());
    assert!(reg.get(PaneId::from_raw(0)).is_none());
}
