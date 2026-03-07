# Versioning & Release Pipeline Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Workspace Version
**File:** `section-01-workspace-version.md` | **Status:** Complete

```
workspace version, Cargo.toml, version.workspace, centralize
workspace.package, inherit, crate version, semver, alpha
```

---

### Section 02: Build-Time Version Injection
**File:** `section-02-build-time-version.md` | **Status:** Complete

```
build.rs, cargo:rustc-env, ORITERM_VERSION, compile-time
git rev-parse, short HEAD, commit hash, build date
ORITERM_CHANNEL, channel detection, dev, nightly, release
env!, version string, version assembly, refactor main, embed_icon
```

---

### Section 03: --version Integration
**File:** `section-03-version-integration.md` | **Status:** Complete

```
clap, --version, version flag, cli, Parser
env!("ORITERM_VERSION"), log::info, startup version
long_version, version_string
```

---

### Section 04: Nightly Workflow
**File:** `section-04-nightly-workflow.md` | **Status:** Complete

```
nightly, GitHub Actions, push to main, rolling release
nightly tag, prerelease, ORITERM_CHANNEL=nightly
build artifact, linux, windows, native build, oriterm-mux
```

---

### Section 05: Release Workflow Update
**File:** `section-05-release-workflow.md` | **Status:** Complete

```
release, tag, v*, ORITERM_CHANNEL=release
release.yml, validate, tagged release, stable
system dependencies, rust-cache, sed validation, oriterm-mux packaging
```

---

### Section 06: AI-Generated Release Notes
**File:** `section-06-ai-release-notes.md` | **Status:** Complete

```
release notes, AI, Claude, Copilot SDK, copilot
generate notes, commit log, PR bodies, conventional commits
nightly notes, release notes, body_path, fallback
COPILOT_GITHUB_TOKEN, claude-sonnet-4.6
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Workspace Version | `section-01-workspace-version.md` |
| 02 | Build-Time Version Injection | `section-02-build-time-version.md` |
| 03 | --version Integration | `section-03-version-integration.md` |
| 04 | Nightly Workflow | `section-04-nightly-workflow.md` |
| 05 | Release Workflow Update | `section-05-release-workflow.md` |
| 06 | AI-Generated Release Notes | `section-06-ai-release-notes.md` |
