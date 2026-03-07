---
section: "03"
title: "--version Integration"
status: complete
goal: "oriterm --version shows the full version string, logged on startup"
inspired_by:
  - "Alacritty cli.rs clap version = env!(\"VERSION\") (alacritty/src/cli.rs:22)"
  - "Alacritty main.rs info!(\"Version {}\") (alacritty/src/main.rs:143)"
depends_on: ["02"]
sections:
  - id: "03.1"
    title: "Wire clap to ORITERM_VERSION"
    status: complete
  - id: "03.2"
    title: "Log version on startup"
    status: complete
  - id: "03.3"
    title: "Completion Checklist"
    status: complete
---

# Section 03: --version Integration

**Status:** Complete
**Goal:** Running `oriterm --version` prints the full version string
(with channel, hash, date). The version is also logged on startup.

**Context:** Currently `cli.rs` uses `#[command(version)]` which pulls
from `CARGO_PKG_VERSION` — just `0.1.0-alpha.3` with no git info. After
Section 02, we have `ORITERM_VERSION` available at compile time.

**Reference implementations:**
- **Alacritty** `src/cli.rs:22`: `#[clap(version = env!("VERSION"))]`
- **Alacritty** `src/main.rs:143`: `info!("Version {}", env!("VERSION"))`

**Depends on:** Section 02 (ORITERM_VERSION env must exist).

---

## 03.1 Wire clap to ORITERM_VERSION

**File(s):** `oriterm/src/cli/mod.rs`

Replace clap's default `version` with the build.rs-provided version string.

Note: `env!("ORITERM_VERSION")` is a compile-time macro that fails compilation
if the env var is missing. This is safe because `build.rs` always runs before
`src/` compilation and unconditionally sets `ORITERM_VERSION`.

- [ ] Change the `Cli` struct attribute. Set both `version` (controls
  `--version` output) and `long_version` (controls `--help` header). Without
  `long_version`, `--help` shows only `CARGO_PKG_VERSION`.
  ```rust
  // Before:
  #[command(name = "oriterm", version, about)]

  // After:
  #[command(
      name = "oriterm",
      version = env!("ORITERM_VERSION"),
      long_version = env!("ORITERM_VERSION"),
      about
  )]
  ```

- [ ] Verify `oriterm --version` output:
  ```
  oriterm 0.1.0-alpha.3-dev (abc1234 2026-03-07)
  ```
- [ ] Verify `oriterm --help` header also shows the full version string

---

## 03.2 Log version on startup

**File(s):** `oriterm/src/main.rs`

Log the version immediately after initializing the logger so it appears at
the top of `oriterm.log`. This is critical for debugging — when a user shares
their log file, the version is the first thing we see.

- [ ] Add version log **immediately after `init_logger()`** (line 40 of
  current `main.rs`) and **before `install_panic_hook()`** (line 41) so
  the version is the first line in `oriterm.log`:
  ```rust
  init_logger();
  log::info!("oriterm {}", env!("ORITERM_VERSION"));
  install_panic_hook();
  ```

---

## 03.3 Completion Checklist

- [ ] `oriterm --version` prints `oriterm X.Y.Z-channel (hash date)`
- [ ] `oriterm.log` first info line shows the version
- [ ] `oriterm --help` shows the full version in the header (not just
  `CARGO_PKG_VERSION`)
- [ ] `./build-all.sh` succeeds
- [ ] `./clippy-all.sh` passes
- [ ] `./test-all.sh` passes
- [ ] CLI tests in `oriterm/src/cli/tests.rs` still pass (existing tests use
  `Cli::try_parse_from` and do not assert on the version string value)

**Exit Criteria:** `oriterm.exe --version` outputs the full version string
matching the format specified in Section 02, and the same string appears in
the log file on startup.
