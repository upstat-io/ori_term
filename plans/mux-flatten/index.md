---
reroute: true
name: "Mux Flatten"
full_name: "Flatten Mux to Pure Pane Server"
status: complete
order: 1
---

# Mux Flatten Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Target API & GUI Session Layer
**File:** `section-01-target-api.md` | **Status:** Complete

```
flat pane server, pane registry, pane CRUD, spawn close resize
session layer, GUI session, tab model, window model, workspace
oriterm session types, local session, client-side state
MuxTab replacement, MuxWindow replacement, TabId, WindowId
```

---

### Section 02: Migrate oriterm to Own Session Types
**File:** `section-02-migrate-oriterm.md` | **Status:** Complete

```
import swap, type migration, MuxTab to local, MuxWindow to local
tab_management, window_management, mux_pump, tab_drag
TermWindow, App, active_pane_context, build_tab_entries
MuxBackend trait, session() method, notification handling
GPU consumer, pane_cache, window_renderer, divider_drag, floating_drag
session sync, spawn flow, close flow, split flow, ID allocation
```

---

### Section 03: Flatten Mux Core
**File:** `section-03-flatten-core.md` | **Status:** Complete

```
InProcessMux, strip tab window, pane-only CRUD
MuxEvent, MuxNotification, pane events, remove tab variants
PaneDirty rename PaneOutput, Alert rename PaneBell
SessionRegistry remove, PaneRegistry keep, IdAllocator
event_pump simplify, tab_ops delete, floating_ops delete
spawn_standalone_pane rename spawn_pane, ClosePaneResult simplify
Phase A tests, Phase B source delete, Phase C struct strip
```

---

### Section 04: Relocate Layout Modules
**File:** `section-04-relocate-layout.md` | **Status:** Complete

```
SplitTree copy, FloatingLayer copy, nav copy, layout compute copy
Rect pixel space, PaneLayout, DividerLayout, hit_test
split_tree, floating, compute, navigation
oriterm layout module, GUI-owned layout
two-phase: copy (04.1-04.4), delete (04.5)
```

---

### Section 05: Flatten Protocol & Server
**File:** `section-05-flatten-protocol.md` | **Status:** Complete (protocol pane-only, server state stripped, MuxBackend flattened, backends updated; transitional session methods kept in trait until oriterm owns session state)

```
wire protocol, MuxPdu, message types, pane-centric protocol
server dispatch, daemon, connection, client router
window_to_client remove, subscriptions simplify, should_exit rewrite
ClientConnection strip, disconnect_client rewrite
snapshot, push notifications, pane-only push, TargetClients
MuxBackend trait simplify, embedded backend, client backend
transport/, TabLayoutUpdate, pushed_layouts, apply_layout_update
notification.rs, notification_to_pdu, rpc_methods strip
```

---

### Section 06: Verification
**File:** `section-06-verification.md` | **Status:** Complete (automated verification done; manual GUI smoke test deferred to next Windows run)

```
test suite, clippy, build, behavioral equivalence
contract tests, e2e tests, unit tests
test migration, test helpers, inject_test_tab, spawn_test_pane
cargo udeps, dependency audit, public API surface
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Target API & GUI Session Layer | `section-01-target-api.md` |
| 02 | Migrate oriterm to Own Session Types | `section-02-migrate-oriterm.md` |
| 03 | Flatten Mux Core | `section-03-flatten-core.md` |
| 04 | Relocate Layout Modules | `section-04-relocate-layout.md` |
| 05 | Flatten Protocol & Server | `section-05-flatten-protocol.md` |
| 06 | Verification | `section-06-verification.md` |
