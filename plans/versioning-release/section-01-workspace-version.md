---
section: "01"
title: "Workspace Version"
status: complete
goal: "Single source of truth for version in root Cargo.toml, all crates inherit"
inspired_by:
  - "Alacritty workspace Cargo.toml (each crate owns version separately)"
  - "Ratatui workspace.package (centralizes edition/rust-version, but NOT version)"
depends_on: []
sections:
  - id: "01.1"
    title: "Add workspace.package.version"
    status: complete
  - id: "01.2"
    title: "Migrate crate versions"
    status: complete
  - id: "01.3"
    title: "Completion Checklist"
    status: complete
---

# Section 01: Workspace Version

**Status:** Complete
**Goal:** All crates in the workspace share a single version defined in the
root `Cargo.toml`. Bumping the version requires editing exactly one line.

**Context:** Currently each crate has its own version (`oriterm` at
`0.1.0-alpha.3`, `oriterm_ipc` at `0.1.0-alpha.1`). This makes version
bumping error-prone and means `CARGO_PKG_VERSION` in the binary crate might
disagree with library crates. Centralizing prevents drift.

**Note:** The `vte` crate under `crates/vte/` is a patched fork of an
external crate and keeps its own upstream version (`0.15.0`). It is NOT
included in the workspace version inheritance.

---

## 01.1 Add workspace.package.version

**File(s):** `Cargo.toml` (workspace root)

Add `workspace.package` with shared metadata to the root `Cargo.toml`.

- [ ] Add `[workspace.package]` section with `version`, `edition`, `license`.
  Insert between `[workspace]` (line 3) and `[workspace.lints.rust]` (line 5)
  to keep all `workspace.*` sections grouped. Section 05's `sed` extraction
  relies on `[workspace.package]` appearing before any other `version =` line.
  ```toml
  [workspace.package]
  version = "0.1.0-alpha.3"
  edition = "2024"
  license = "MIT"
  ```

---

## 01.2 Migrate crate versions

**File(s):** `oriterm/Cargo.toml`, `oriterm_core/Cargo.toml`, `oriterm_mux/Cargo.toml`, `oriterm_ui/Cargo.toml`, `oriterm_ipc/Cargo.toml`

Replace per-crate version/edition/license with workspace inheritance. Do NOT
inherit `description` — each crate has a unique description.

- [ ] In each crate's `[package]`, replace:
  ```toml
  # Before:
  version = "0.1.0-alpha.3"
  edition = "2024"
  license = "MIT"

  # After:
  version.workspace = true
  edition.workspace = true
  license.workspace = true
  ```
- [ ] Bring `oriterm_ipc` up from `0.1.0-alpha.1` to the shared version
  (it was behind — this is the fix, not a regression). Its `[lints.rust]` and
  `[lints.clippy]` sections remain local (they override `unsafe_code = "allow"`);
  only version/edition/license inherit.
- [ ] Verify `cargo metadata --format-version=1 | jq '.packages[].version'`
  shows the same version for all workspace crates (excluding `vte`)
- [ ] Verify `Cargo.lock` regenerates cleanly and commit it alongside the
  `Cargo.toml` changes
- [ ] Verify `cargo build` succeeds

---

## 01.3 Completion Checklist

- [ ] Root `Cargo.toml` has `[workspace.package]` with `version`
- [ ] All 5 workspace crates use `version.workspace = true`
- [ ] `crates/vte/Cargo.toml` is unchanged (keeps `0.15.0`)
- [ ] `cargo build --target x86_64-pc-windows-gnu` succeeds
- [ ] `./clippy-all.sh` passes
- [ ] `./test-all.sh` passes
- [ ] `Cargo.lock` changes committed alongside `Cargo.toml` changes

**Exit Criteria:** `cargo metadata` reports identical versions for all
workspace crates. A single edit to `Cargo.toml` line changes the version
everywhere.
