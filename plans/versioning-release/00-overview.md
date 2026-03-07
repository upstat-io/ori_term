---
plan: "versioning-release"
title: "Versioning & Nightly Release Pipeline"
status: complete
supersedes: []
references:
  - "Alacritty build.rs version injection (alacritty/build.rs)"
  - "Ghostty channel-based versioning (src/build/Config.zig, src/build/GitVersion.zig)"
  - "Rust compiler --version format (rustc X.Y.Z-channel (hash date))"
  - "WezTerm nightly pipeline (.github/workflows/gen_*_continuous.yml)"
  - "ori_lang auto-release.yml (AI-generated release notes via Copilot SDK + Claude)"
---

# Versioning & Nightly Release Pipeline

## Mission

Establish a versioning system where every push to `main` produces a uniquely
identified nightly build published to GitHub Releases, and tagged milestone
releases (alpha/beta/stable) produce clean versioned builds. The binary's
`--version` output unambiguously identifies the channel, commit, and date of
any given build.

## Architecture

```
                       Cargo.toml
                    workspace.version
                    "0.1.0-alpha.3"
                          |
                      build.rs
                     (compile time)
                    /      |      \
           CARGO_PKG    git rev   ORITERM_CHANNEL
           _VERSION    short HEAD   env var
                    \      |      /
                  ORITERM_VERSION env
                          |
            +--------------------------+
            |  Channel detection:      |
            |  release -> clean ver    |
            |  nightly -> ver-nightly  |
            |  (unset) -> ver-dev      |
            +--------------------------+
                          |
                    clap --version
                          |
            +--------------------------+
            | oriterm 0.1.0-alpha.3    |
            |   (abc1234 2026-03-07)   |
            +--------------------------+
```

## Design Principles

**Rust compiler convention.** The `--version` format follows the pattern
established by `rustc` and widely understood by the Rust ecosystem:
`name version-channel (hash date)`. This is instantly parseable by humans
and machines, and channel is unambiguous from the version string alone.

**Build-time baking (Alacritty pattern).** Version is a compile-time constant
injected by `build.rs` via `cargo:rustc-env`. No runtime computation, no
config files, no OnceLock indirection. The version string is a `&'static str`
baked into the binary. This is the simplest correct approach.

**Automatic channel detection (Ghostty pattern).** The CI pipeline sets
`ORITERM_CHANNEL` to control the suffix. Local dev builds default to `-dev`.
No manual version bumping for nightlies. The Cargo.toml version is bumped
only for milestones.

## --version Output Format

```
oriterm 0.1.0-alpha.3 (abc1234 2026-03-07)          # tagged release
oriterm 0.1.0-alpha.3-nightly (abc1234 2026-03-07)  # CI nightly from main
oriterm 0.1.0-alpha.3-dev (abc1234 2026-03-07)      # local dev build
oriterm 0.1.0-alpha.3-dev (unknown)                  # build without git
```

## Section Dependency Graph

```
  01 Workspace Version
          |
  02 Build-Time Version
          |
  03 --version Integration
         / \
        /   \
  04 Nightly   05 Release
  Workflow     Workflow Update
        \       /
    06 AI Release Notes
```

- Sections 01-03 are sequential (each builds on the prior).
- Sections 04-05 are independent of each other but require 01-03.
- Section 06 requires both 04 and 05 (adds release notes to both workflows).

## Implementation Sequence

```
Phase 0 - Centralize Version
  +-- 01: Workspace version in root Cargo.toml, all crates inherit

Phase 1 - Build-Time Injection
  +-- 02: build.rs assembles version string from git + channel env
  +-- 03: clap uses env!("ORITERM_VERSION"), log on startup

Phase 2 - CI Pipelines
  +-- 04: Nightly workflow (push to main -> rolling release)
  +-- 05: Update existing release workflow (set ORITERM_CHANNEL=release)
  Gate: `oriterm --version` shows correct channel for all build types

Phase 3 - Release Notes
  +-- 06: AI-generated release notes via Copilot SDK + Claude
  Gate: Both nightly and tagged releases have AI-generated release notes
```

**Why this order:**
- Phase 0 is a pure Cargo.toml change, no behavioral impact.
- Phase 1 must come before Phase 2 because the CI needs the build.rs
  infrastructure to exist before it can set ORITERM_CHANNEL.

## Out of Scope (Future Work)

- **`oriterm-mux` binary version:** The mux daemon binary (`oriterm_mux/src/bin/oriterm_mux.rs`)
  does not currently have CLI argument parsing or `--version` support. Adding
  version reporting to the mux binary is deferred — it inherits the workspace
  version in `Cargo.toml` but has no `build.rs` to inject the full version
  string. The mux binary IS packaged in nightly and release archives (sections
  04/05); it just lacks its own `--version` flag. Add this when the mux daemon
  gets its own CLI interface.
- **macOS release binaries:** The CI runs tests on macOS but release/nightly
  workflows only produce Linux and Windows binaries. Add a `build-macos` job
  when macOS platform support is ready.
- **CHANGELOG file:** This plan generates release notes for GitHub Releases
  but does not maintain a `CHANGELOG.md` file. Add changelog file generation
  when the project needs a static changelog (e.g., for package managers).
- **Version bump automation:** Bumping the workspace version for milestones
  remains a manual edit to one line in root `Cargo.toml`. A `cargo-release`
  or similar tool integration is deferred.

## Estimated Effort

| Section | Est. Lines | Complexity | Depends On |
|---------|-----------|------------|------------|
| 01 Workspace Version | ~10 changed | Low | -- |
| 02 Build-Time Version | ~80 new | Medium | 01 |
| 03 --version Integration | ~15 changed | Low | 02 |
| 04 Nightly Workflow | ~100 new | Medium | 03 |
| 05 Release Workflow Update | ~60 changed | Low | 03 |
| 06 AI Release Notes | ~150 new | Medium | 04, 05 |
| **Total new** | **~400** | | |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Workspace Version | `section-01-workspace-version.md` | Complete |
| 02 | Build-Time Version Injection | `section-02-build-time-version.md` | Complete |
| 03 | --version Integration | `section-03-version-integration.md` | Complete |
| 04 | Nightly Workflow | `section-04-nightly-workflow.md` | Complete |
| 05 | Release Workflow Update | `section-05-release-workflow.md` | Complete |
| 06 | AI-Generated Release Notes | `section-06-ai-release-notes.md` | Complete |
