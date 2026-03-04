---
section: "01"
title: Enrich PaneSnapshot
status: complete
goal: Add stable_row_base and cols fields to PaneSnapshot so client-side selection math works
sections:
  - id: "01.1"
    title: Add stable_row_base field
    status: complete
  - id: "01.2"
    title: Add cols field
    status: complete
  - id: "01.3"
    title: Populate new fields in build_snapshot
    status: complete
  - id: "01.4"
    title: Update frame extraction from snapshot
    status: complete
  - id: "01.5"
    title: Completion Checklist
    status: complete
---

# Section 01: Enrich PaneSnapshot

**Status:** 📋 Planned
**Goal:** Add fields to `PaneSnapshot` that client-side code needs for selection, mark mode, and viewport math — without changing the rendering pipeline yet.

**Crate:** `oriterm_mux`
**Key files:**
- `oriterm_mux/src/protocol/snapshot.rs` — PaneSnapshot struct
- `oriterm_mux/src/server/snapshot.rs` — `build_snapshot()` function (or wherever snapshots are constructed)
- `oriterm/src/gpu/extract/from_snapshot/mod.rs` — `extract_frame_from_snapshot*` functions

---

## 01.1 Add `stable_row_base` Field

The snapshot currently has `scrollback_len` and `display_offset` separately. Client-side selection math needs `stable_row_base` — the absolute row index of the first visible row. Making this explicit avoids error-prone subtraction at every call site.

**Semantics:** `stable_row_base` must align with `StableRowIndex` — it represents the total number of rows that have existed below the viewport base. When scrollback eviction is active (scrollback buffer is full and lines are being discarded), `scrollback_len` caps at the maximum but `stable_row_base` keeps growing. The correct computation is the same one `StableRowIndex::from_absolute()` uses in `oriterm_core` — verify by reading that function. The naive `scrollback_len - display_offset` is only correct when no eviction has occurred.

**File:** `oriterm_mux/src/protocol/snapshot.rs`

- [ ] Add `pub stable_row_base: u64` field to `PaneSnapshot`
  - Doc comment: "Absolute row index of the first viewport row. Matches `StableRowIndex` semantics: accounts for scrollback eviction, not just current buffer length."
  - Placed after `display_offset` (related fields grouped together)

---

## 01.2 Add `cols` Field

Currently `cols` must be inferred from `cells[0].len()`, which is fragile (empty snapshot, ragged rows). Make it explicit.

**File:** `oriterm_mux/src/protocol/snapshot.rs`

- [ ] Add `pub cols: u16` field to `PaneSnapshot`
  - Doc comment: "Grid column count. Explicit to avoid fragile `cells[0].len()` inference."
  - Placed after `display_offset` / `stable_row_base`

---

## 01.3 Populate New Fields in `build_snapshot`

Find the server-side code that constructs `PaneSnapshot` and populate the new fields.

**File:** `oriterm_mux/src/server/snapshot.rs` (or `oriterm_mux/src/server/dispatch.rs` — locate `build_snapshot` or `PaneSnapshot { ... }` construction)

- [ ] Locate all `PaneSnapshot` construction sites
- [ ] Populate `stable_row_base` from `renderable_content().stable_row_base` (or equivalent terminal-provided stable base), not by reconstructing from `scrollback_len/display_offset`
- [ ] Populate `cols` from `grid.cols() as u16`
- [ ] Verify any test helpers or test snapshot builders also set these fields

---

## 01.4 Update Frame Extraction from Snapshot

The GPU frame extraction functions (`extract_frame_from_snapshot`, `extract_frame_from_snapshot_into`) may compute `stable_row_base` internally. Update them to use the snapshot field directly.

**File:** `oriterm/src/gpu/extract/from_snapshot/mod.rs`

- [ ] Replace internal `stable_row_base` computation with `snapshot.stable_row_base`
- [ ] Replace `cells[0].len()` with `snapshot.cols as usize` where applicable
- [ ] Verify extracted frame's `content.stable_row_base` matches

---

## 01.5 Completion Checklist

- [ ] `PaneSnapshot` has `stable_row_base: u64` and `cols: u16` fields
- [ ] All snapshot construction sites populate both fields
- [ ] Frame extraction uses the new fields
- [ ] `./build-all.sh` passes
- [ ] `./clippy-all.sh` passes
- [ ] `./test-all.sh` passes
- [ ] Existing daemon rendering unchanged (manual verify or test)

**Exit Criteria:** PaneSnapshot carries all data needed for client-side viewport math. No behavioral changes yet — this is pure data enrichment.
