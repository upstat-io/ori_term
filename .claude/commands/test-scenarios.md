---
name: test-scenarios
description: Find missing test scenarios by comparing against reference terminal emulator repos
allowed-tools: Read, Grep, Glob, Task, Bash
---

# Test Scenarios Gap Analysis

Compare the test coverage for recently added or modified code against established terminal emulator repos (Alacritty, WezTerm, Ghostty, Ratatui, Crossterm) and identify missing test scenarios.

## Target

`$ARGUMENTS` specifies the scope to analyze. There are three modes:

### Path Mode (explicit file/directory targets)
- `/test-scenarios src/app/mouse_selection/` — analyze tests for mouse selection
- `/test-scenarios src/gpu/atlas.rs` — analyze tests for glyph atlas
- `/test-scenarios oriterm_core/src/selection/` — analyze tests for selection core

### Commit Mode (use a commit as scope selector)
- `/test-scenarios last commit` — analyze tests for files touched by the most recent commit
- `/test-scenarios last 3 commits` — analyze files touched by the last N commits

### No argument (default)
- Defaults to `last commit` mode — analyzes files touched by the most recent commit.

---

## Execution

### Step 1: Identify Scope

**Path mode:** Use the provided paths directly.

**Commit mode:**
1. Run `git diff --name-only HEAD~N..HEAD` to get changed `.rs` files
2. Expand to include the full module(s) those files belong to
3. Identify the corresponding `tests.rs` files

### Step 2: Read Our Tests

Read the test files for the target modules. Catalog:
- What scenarios are tested
- What test helpers exist
- What aspects of the code are exercised

### Step 3: Launch Reference Repo Research Agent

Launch a single Explore agent to search the reference repos for comparable test patterns:

```
Task(
  subagent_type: "Explore",
  description: "Find ref test gaps",
  prompt: <see Agent template below>
)
```

### Step 4: Compare and Report

Compare our test coverage against the reference repos. Report:

```
## Test Scenarios Gap Analysis: {scope}

### Current Coverage
- {count} tests covering: {brief list of what's tested}

### Missing Scenarios Found in Reference Repos

#### High Priority (likely to catch real bugs)
1. **{Scenario name}** — {description}
   - Found in: {repo} `{file path}`
   - Why it matters: {brief rationale}

2. ...

#### Medium Priority (edge cases and robustness)
1. ...

#### Low Priority (nice to have)
1. ...

### Summary
- Current: {N} tests
- Missing: {M} scenarios ({H} high, {M} medium, {L} low priority)
```

### Step 5: Ask What to Do

Use AskUserQuestion with options:
1. **Add all high-priority tests (Recommended)** — implement the high-priority missing tests now
2. **Add all missing tests** — implement all identified missing tests
3. **Pick specific tests** — choose which tests to add
4. **Just the report** — done, no code changes needed

### Step 6: Implement (if requested)

Add the missing tests following the project's test organization rules:
- Tests in sibling `tests.rs` files
- `super::` imports for parent module items
- No module wrapper in test files
- Run `cargo test` to verify, then `./clippy-all.sh`

---

## Explore Agent Prompt Template

```
You are searching reference terminal emulator repos for test patterns that cover
{DOMAIN/MODULE DESCRIPTION}.

Our current implementation is in: {ORI_TERM FILES}
Our current tests cover: {BRIEF SUMMARY OF EXISTING TESTS}

Search these reference repos at ~/projects/reference_repos/console_repos/ for
test files covering similar functionality:

1. **Alacritty** — search `alacritty_terminal/src/` for relevant test modules
2. **WezTerm** — search `term/src/` and `wezterm-gui/src/` for relevant tests
3. **Ghostty** — search `src/terminal/` for relevant test files (Zig, `test` blocks)

For each repo, find test files related to {DOMAIN} and extract:
- Test function names and what scenarios they cover
- Edge cases being tested (boundary conditions, error paths, empty inputs)
- Integration-level test patterns (multi-step workflows)
- Any test helpers or fixtures that enable thorough testing

Focus on TEST SCENARIOS (what's being verified), not implementation details.
Compare against our existing coverage and identify gaps.

Produce a structured list:
## Reference Test Scenarios: {Domain}

### Alacritty
- {test name}: {what it verifies} — {do we have this? yes/no}
...

### WezTerm
...

### Ghostty
...

### Scenarios We're Missing
Ranked by priority (high = likely to catch real bugs, medium = edge cases,
low = nice to have):

**High:**
1. {scenario}: {why it matters}

**Medium:**
1. ...

**Low:**
1. ...

RULES:
- Actually READ test files, don't just search for names
- Focus on SCENARIOS not implementation — what's being verified matters more than how
- Only flag scenarios as "missing" if they're genuinely applicable to our code
- Don't flag tests for features we haven't implemented yet
- Keep output under 200 lines
```

---

## Domain Mapping Hints

When the target path contains these patterns, focus the reference search accordingly:

| Path pattern | Search domain | Alacritty | WezTerm | Ghostty |
|---|---|---|---|---|
| `selection` | selection, mouse | `selection.rs` | `selection.rs` | `Selection.zig` |
| `mouse` | mouse input, click, drag | `input/mod.rs` | `mouseevent.rs` | `mouse.zig` |
| `grid` | grid buffer, storage | `grid/` | `screen.rs` | `Screen.zig`, `PageList.zig` |
| `gpu`, `render`, `prepare` | rendering | `renderer/` | `renderstate.rs` | `renderer/` |
| `key_encoding` | keyboard input | `input/keyboard.rs` | `keyevent.rs` | `key_encode.zig` |
| `pty` | PTY management | `tty/` | `pty/` | `pty.zig` |
| `term_handler`, `vte` | VTE/escape handling | `term/mod.rs` | `terminalstate/` | `Terminal.zig` |
| `font`, `atlas` | font/glyph | `text/glyph_cache.rs` | `glyphcache.rs` | `font/` |
| `palette`, `color` | color system | `term/color.rs` | `color.rs` | `color.zig` |
