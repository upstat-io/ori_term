---
section: "02"
title: "Build-Time Version Injection"
status: complete
goal: "build.rs assembles a full version string with channel, git hash, and date"
inspired_by:
  - "Alacritty build.rs git hash injection (alacritty/build.rs:8-40)"
  - "Ghostty GitVersion.zig detection (src/build/GitVersion.zig)"
  - "WezTerm wezterm-version/build.rs (.tag file + git fallback)"
depends_on: ["01"]
sections:
  - id: "02.1"
    title: "Version assembly in build.rs"
    status: complete
  - id: "02.2"
    title: "Graceful degradation"
    status: complete
  - id: "02.3"
    title: "Completion Checklist"
    status: complete
---

# Section 02: Build-Time Version Injection

**Status:** Complete
**Goal:** `oriterm/build.rs` emits a `ORITERM_VERSION` compile-time environment
variable containing the full version string. The format follows the Rust
compiler convention: `version-channel (hash date)`.

**Context:** Currently `--version` shows only the Cargo.toml version
(`0.1.0-alpha.3`) with no git info and no channel indicator. There is no way
to tell if a binary is a nightly, a release, or a local dev build.

**Reference implementations:**
- **Alacritty** `alacritty/build.rs`: Reads `CARGO_PKG_VERSION`, appends git
  short hash, exports as `cargo:rustc-env=VERSION`. Simple and proven.
- **Ghostty** `src/build/GitVersion.zig`: Detects branch, tag, hash, dirty
  state. Channel derived from presence of pre-release string.
- **Rust compiler**: `rustc 1.78.0-nightly (a4472498d 2024-02-15)` — the
  gold standard format.

**Depends on:** Section 01 (workspace version must exist so
`CARGO_PKG_VERSION` is correct).

---

## 02.1 Version assembly in build.rs

**File(s):** `oriterm/build.rs`

Extend the existing `build.rs` (which already handles icon embedding) to
assemble and export the version string.

The existing `main()` is 65 lines. Adding version assembly + rerun-if-changed
would push it to ~81 lines, violating the 50-line function limit. Extract icon
code into helpers first.

Integration point: insert version code between `let assets = ...` (line 7) and
the icon embedding block (line 9). No new imports needed (`Command` and `PathBuf`
are already imported).

- [ ] **Refactor first:** Extract the Windows icon embedding code (lines 9-54
  of current `build.rs`) into `embed_icon(out_dir: &str, assets: &Path)`.
  Extract the PNG decode block (lines 56-68) into
  `decode_icon_png(out_dir: &str, assets: &Path)`. This keeps `main()` under
  50 lines after adding version assembly.
- [ ] Add version assembly to `main()` in `build.rs`, **after `let assets`
  and before the icon embedding call** (runs unconditionally on all platforms):
  ```rust
  // Version assembly — must come before any early returns.
  let version = assemble_version();
  println!("cargo:rustc-env=ORITERM_VERSION={version}");
  ```

- [ ] Implement `assemble_version()`:
  ```rust
  /// Build the full version string.
  ///
  /// Format: `{cargo_version}[-{channel}] ({hash} {date})`
  ///
  /// Channel is derived from the `ORITERM_CHANNEL` env var:
  /// - "release" -> no suffix (clean version)
  /// - "nightly" -> "-nightly" suffix
  /// - unset/other -> "-dev" suffix
  ///
  /// If git is unavailable, the parenthetical shows "(unknown)".
  fn assemble_version() -> String {
      // NOTE: must use std::env::var() (runtime), NOT env!() (compile-time).
      // env!() reads the env for the build.rs binary itself, which has no
      // CARGO_PKG_VERSION. std::env::var() reads what Cargo sets at runtime.
      let base = std::env::var("CARGO_PKG_VERSION")
          .expect("CARGO_PKG_VERSION must be set by Cargo");

      let channel = match std::env::var("ORITERM_CHANNEL").as_deref() {
          Ok("release") => "",
          Ok("nightly") => "-nightly",
          _ => "-dev",
      };

      let git_info = git_info().unwrap_or_else(|| "unknown".to_owned());

      format!("{base}{channel} ({git_info})")
  }
  ```

- [ ] Implement `git_info()`:
  ```rust
  /// Query git for short hash and commit date.
  ///
  /// Returns `Some("abc1234 2026-03-07")` or `None` if git is unavailable.
  fn git_info() -> Option<String> {
      let hash = Command::new("git")
          .args(["rev-parse", "--short=7", "HEAD"])
          .output()
          .ok()
          .filter(|o| o.status.success())
          .and_then(|o| String::from_utf8(o.stdout).ok())
          .map(|s| s.trim().to_owned())?;

      // %cs = committer date, short format (YYYY-MM-DD). Supported since Git 2.21.
      let date = Command::new("git")
          .args(["show", "-s", "--format=%cs", "HEAD"])
          .output()
          .ok()
          .filter(|o| o.status.success())
          .and_then(|o| String::from_utf8(o.stdout).ok())
          .map(|s| s.trim().to_owned())
          .unwrap_or_else(|| "unknown-date".to_owned());

      Some(format!("{hash} {date}"))
  }
  ```

- [ ] Add `rerun-if-changed` for `.git/HEAD` and the current ref so the
  version updates on new commits. Place right after `cargo:rustc-env=ORITERM_VERSION`,
  before icon embedding. Reuse the existing `workspace_root` variable (line 6
  of current `build.rs`) -- do not redeclare it.
  ```rust
  // Rebuild when the git HEAD changes (new commit, branch switch).
  // (workspace_root is declared at the top of main(), shared with icon code)
  let git_head = workspace_root.join(".git/HEAD");
  if git_head.exists() {
      println!("cargo:rerun-if-changed={}", git_head.display());
      // Also watch the ref that HEAD points to (for new commits).
      if let Ok(head_content) = std::fs::read_to_string(&git_head) {
          if let Some(ref_path) = head_content.trim().strip_prefix("ref: ") {
              let ref_file = workspace_root.join(".git").join(ref_path);
              if ref_file.exists() {
                  println!("cargo:rerun-if-changed={}", ref_file.display());
              }
          }
      }
  }
  // Rebuild when the channel env var changes.
  println!("cargo:rerun-if-env-changed=ORITERM_CHANNEL");
  ```

  **Worktree limitation:** In git worktrees, `.git` is a file (not a directory),
  so `git_head.exists()` returns false and the rerun-if-changed won't trigger
  on new commits. `git rev-parse` still works. A `cargo clean` fixes stale
  versions in worktree builds.

---

## 02.2 Graceful degradation

The version assembly must never fail the build. Every step has a fallback.

- [ ] If `ORITERM_CHANNEL` is unset: default to `"-dev"` (not an error)
- [ ] If `git` is not installed: show `(unknown)` instead of hash+date
- [ ] If `.git/` doesn't exist (e.g., source tarball): show `(unknown)`
- [ ] If git commands fail (shallow clone, corrupted repo): show `(unknown)`.
  Note: detached HEAD (common in CI) works fine -- `git rev-parse --short HEAD`
  succeeds and the rerun-if-changed correctly skips the ref file.
- [ ] Verify: a build in a directory with no `.git/` still succeeds and
  produces version `0.1.0-alpha.3-dev (unknown)`

---

## 02.2b Testing the version string

Build scripts (`build.rs`) are separate compilation units and cannot have
`#[test]` blocks. Verify by inspecting the compiled binary:

- [ ] Verify the baked version string in the binary:
  ```bash
  cargo build --target x86_64-pc-windows-gnu
  strings target/x86_64-pc-windows-gnu/debug/oriterm.exe | grep -E '^\d+\.\d+\.\d+'
  ```
- [ ] Verify channel override works:
  ```bash
  ORITERM_CHANNEL=nightly cargo build --target x86_64-pc-windows-gnu
  strings target/x86_64-pc-windows-gnu/debug/oriterm.exe | grep nightly
  ```
- [ ] Verify no-git fallback by code inspection: confirm `git_info()` returns
  `None` when `Command::new("git")` fails (the `ok()` + `filter()` chain
  handles this).

---

## 02.3 Completion Checklist

- [ ] `build.rs` `main()` is under 50 lines (icon embedding extracted to helpers)
- [ ] `build.rs` exports `ORITERM_VERSION` as a compile-time env var
- [ ] Local build: `ORITERM_VERSION` = `0.1.0-alpha.3-dev (abc1234 2026-03-07)`
- [ ] With `ORITERM_CHANNEL=nightly`: `0.1.0-alpha.3-nightly (abc1234 2026-03-07)`
- [ ] With `ORITERM_CHANNEL=release`: `0.1.0-alpha.3 (abc1234 2026-03-07)`
- [ ] Without git: `0.1.0-alpha.3-dev (unknown)`
- [ ] Version updates when a new commit is made (rerun-if-changed works)
- [ ] `./build-all.sh` succeeds
- [ ] `./clippy-all.sh` passes (no warnings in build.rs)
- [ ] `./test-all.sh` passes

**Exit Criteria:** `env!("ORITERM_VERSION")` compiles in the binary crate and
returns the correct format for all channel/git combinations.
