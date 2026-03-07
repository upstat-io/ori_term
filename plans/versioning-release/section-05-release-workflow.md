---
section: "05"
title: "Release Workflow Update"
status: complete
goal: "Existing release.yml sets ORITERM_CHANNEL=release so tagged builds show clean version"
inspired_by:
  - "Ghostty release-tag.yml (-Dversion-string passed to build)"
depends_on: ["03"]
sections:
  - id: "05.1"
    title: "Set ORITERM_CHANNEL in release workflow"
    status: complete
  - id: "05.2"
    title: "Verify tag-version validation still works"
    status: complete
  - id: "05.3"
    title: "Completion Checklist"
    status: complete
---

# Section 05: Release Workflow Update

**Status:** Complete
**Goal:** The existing `release.yml` sets `ORITERM_CHANNEL=release` so
tagged builds produce clean version strings without a channel suffix.

**Context:** The current `release.yml` validates that the git tag matches
the Cargo.toml version. The primary change is adding the channel env var so
the binary doesn't show `-dev` in its version string. While touching this
file, also fix pre-existing gaps: missing Linux system dependencies, missing
`rust-cache`, missing `oriterm-mux` in release archives, and a brittle
version extraction grep.

**Depends on:** Section 03 (requires sections 01-03: workspace version,
build.rs channel detection, and clap integration).

---

## 05.1 Set ORITERM_CHANNEL in release workflow

**File(s):** `.github/workflows/release.yml`

- [ ] Add `ORITERM_CHANNEL: release` to the top-level `env:` block:
  ```yaml
  env:
    CARGO_TERM_COLOR: always
    ORITERM_CHANNEL: release
  ```
  This propagates to all jobs, so both `build-linux` and `build-windows`
  produce binaries with clean version strings.

- [ ] Verify: a tagged release binary shows:
  ```
  oriterm 0.1.0-alpha.3 (abc1234 2026-03-07)
  ```
  (no `-dev`, no `-nightly`)

- [ ] Add system dependency installation to the `build-linux` job. The
  existing `release.yml` is missing this step (pre-existing bug). wgpu and
  winit require X11/Wayland/EGL headers to compile on Linux:
  ```yaml
  - name: Install system dependencies
    run: |
      sudo apt-get update
      sudo apt-get install -y \
        pkg-config libx11-dev libxi-dev libxcursor-dev \
        libxrandr-dev libxinerama-dev libwayland-dev \
        libxkbcommon-dev libegl-dev libvulkan-dev
  ```

- [ ] Add `Swatinem/rust-cache@v2` to both build jobs for faster CI builds
  (matches nightly workflow pattern):
  ```yaml
  - uses: Swatinem/rust-cache@v2
  ```

- [ ] Update the `build-linux` Package step to include `oriterm-mux` (the
  existing `release.yml` only packages `oriterm`, omitting the mux daemon):
  ```yaml
  - name: Strip binaries
    run: |
      strip target/release/oriterm
      strip target/release/oriterm-mux
  - name: Package
    run: |
      cd target/release
      tar -czvf ../../oriterm-${{ github.ref_name }}-linux-x86_64.tar.gz oriterm oriterm-mux
      cd ../..
  ```
- [ ] Update the `build-windows` Package step to include `oriterm-mux.exe`:
  ```yaml
  - name: Package
    run: |
      cd target/x86_64-pc-windows-msvc/release
      7z a ../../../oriterm-${{ github.ref_name }}-windows-x86_64.zip oriterm.exe oriterm-mux.exe
      cd ../../..
  ```

---

## 05.2 Verify tag-version validation still works

**File(s):** `.github/workflows/release.yml`

The existing validation step extracts the version from Cargo.toml and
compares it to the tag. Since we moved to workspace version, verify the
`grep` pattern still works.

- [ ] The validation step greps `Cargo.toml` for version. After Section 01,
  the version lives in `[workspace.package]` not `[package]`. Verify the
  grep pattern `grep -E '^version\s*=' Cargo.toml | head -1` still picks up
  the workspace version. **This works because the root `Cargo.toml` has no
  `[package]` section — only `[workspace.package]`.**
- [ ] As a safety measure, replace the brittle grep with a `sed`-based
  extraction that targets `[workspace.package]` specifically:
  ```bash
  # Extract version from [workspace.package] section specifically.
  # Dots escaped so sed treats them as literal; POSIX-compatible syntax.
  CARGO_VERSION=$(sed -n '/\[workspace\.package\]/,/^\[/{ s/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p }' Cargo.toml)
  ```
  This is more resilient to future Cargo.toml restructuring.

---

## 05.3 Completion Checklist

- [ ] `release.yml` has `ORITERM_CHANNEL: release` in env
- [ ] Tag-version validation still works with workspace version
- [ ] Tagged release binary shows clean version (no channel suffix)
- [ ] Nightly workflow is unaffected (still uses `ORITERM_CHANNEL: nightly`)
- [ ] `./build-all.sh` still works locally (no ORITERM_CHANNEL = dev default)
- [ ] Linux build job has system dependency installation step
- [ ] Both build jobs have `rust-cache` for faster CI builds
- [ ] Tag-version validation uses `sed` targeting `[workspace.package]`
- [ ] Both platform archives include `oriterm-mux` binary alongside `oriterm`

**Exit Criteria:** Tag a test release (e.g., on a branch), verify the
release workflow sets the channel correctly and the binary version string
has no `-dev` or `-nightly` suffix.
