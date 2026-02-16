---
name: impl-hygiene-review
description: Review implementation hygiene at module boundaries. NOT architecture or code style — purely plumbing quality.
allowed-tools: Read, Grep, Glob, Task, Bash, EnterPlanMode
---

# Implementation Hygiene Review

Review implementation hygiene against `.claude/rules/impl-hygiene.md` and generate a plan to fix violations.

**Implementation hygiene is NOT architecture** (design decisions are made) **and NOT code style** (naming, comments, formatting). It's the plumbing layer — module boundaries, data flow, error propagation, rendering discipline.

## Target

`$ARGUMENTS` specifies the boundary or scope to review. There are two modes:

### Path Mode (explicit file/directory targets)
- `/impl-hygiene-review src/grid src/term_handler.rs` — review grid↔VTE handler boundary
- `/impl-hygiene-review src/gpu/` — review GPU rendering internals
- `/impl-hygiene-review src/app.rs src/tab.rs` — review app↔tab boundary
- `/impl-hygiene-review src/` — review all module boundaries

### Commit Mode (use a commit as a scope selector)
- `/impl-hygiene-review last commit` — review files touched by the most recent commit
- `/impl-hygiene-review last 3 commits` — review files touched by the last N commits
- `/impl-hygiene-review <commit-hash>` — review files touched by a specific commit

**CRITICAL: Commits are scope selectors, NOT content filters.** The commit determines WHICH files and areas to review. Once the files are identified, review them completely — report ALL hygiene findings in those files, regardless of whether the finding is "related to" or "caused by" the commit. The commit is a lens to focus on a region of the codebase, nothing more. Do NOT annotate findings with whether they relate to the commit. Do NOT deprioritize or exclude findings because they predate the commit.

**Commit scoping procedure:**
1. Use `git diff --name-only HEAD~N..HEAD` (or appropriate range) to get the list of changed `.rs` files
2. Expand to include the full module(s) those files belong to (e.g., if `src/gpu/atlas.rs` was touched, include all of `src/gpu/`)
3. Proceed with the standard review process using those modules as the target

If no argument: default to `last commit` mode (review files touched by the most recent commit).

## Execution

### Step 1: Load Rules

Read `.claude/rules/impl-hygiene.md` to have the full rule set in context.

### Step 2: Map the Boundary

Identify the module boundary being reviewed:
1. What types cross the boundary? (cells, grid state, render params, events)
2. What functions form the interface? (entry points, draw calls, event handlers)
3. What data flows across? (grid cells, palette colors, font metrics, input events)

For each module in the target, read the key files to understand the public API surface.

### Step 3: Trace Data Flow

Follow the data from producer to consumer:
1. **Read the producer's output types** — What does the upstream module emit?
2. **Read the consumer's input handling** — How does the downstream module receive and process it?
3. **Check the boundary types** — Are they minimal? Do they carry unnecessary baggage?
4. **Check ownership** — Is data moved, borrowed, or cloned? Are clones necessary?

### Step 4: Audit Each Rule Category

**Module Boundary Discipline:**
- [ ] Data flows one way? (no callbacks to earlier layer, no reaching back)
- [ ] No circular imports between modules?
- [ ] Boundary types are minimal? (only what's needed crosses)
- [ ] Clean ownership transfer? (borrow for rendering, move for ownership changes)
- [ ] No layer bleeding? (grid doesn't render, renderer doesn't parse VTE)

**Data Flow:**
- [ ] Zero-copy where possible? (cell references, not cell copies)
- [ ] No allocation in hot paths? (render loop, VTE input, key encoding)
- [ ] Newtypes for IDs? (`TabId`, not bare `u64`)
- [ ] Instance buffers reused across frames?
- [ ] Glyph cache avoids redundant rasterization?

**Error Handling at Boundaries:**
- [ ] No panics on user input? (bad escapes, invalid UTF-8, unexpected keys)
- [ ] PTY errors recoverable? (close tab, don't crash app)
- [ ] GPU errors surfaced? (surface lost, device lost → recover or report)
- [ ] Config errors fall back to defaults?

**Rendering Discipline:**
- [ ] Frame building is pure computation? (no side effects on Grid/Tab/App)
- [ ] No state mutation during render?
- [ ] Color resolution happens once per frame, not per pass?
- [ ] Atlas misses are handled without blocking the frame?

**Event Flow:**
- [ ] Events flow through the event loop? (no bypassing `TermEvent`)
- [ ] Input dispatch is a decision tree? (one handler per event, no fallthrough)
- [ ] State transitions use enums, not booleans?
- [ ] Redraw requests coalesced?

**Platform Abstraction:**
- [ ] `#[cfg()]` at module level, not scattered inline?
- [ ] Grid, VTE handler, selection, search are platform-independent?

### Step 5: Compile Findings

Organize findings by boundary/interface, categorized as:

- **LEAK** — Data or control flow crossing a boundary it shouldn't (layer bleeding, backward reference, panic on user input)
- **WASTE** — Unnecessary allocation, clone, or transformation at boundary (extra copy, redundant resolution, per-frame allocation)
- **EXPOSURE** — Internal state leaking through boundary types (app state in render params, grid internals in input handler)
- **NOTE** — Observation, not actionable (acceptable tradeoff, documented exception)

### Step 6: Generate Plan

Use **EnterPlanMode** to create a fix plan. The plan should:

1. List every LEAK, WASTE, and EXPOSURE finding with `file:line` references
2. Group by boundary (e.g., "app↔tab", "tab↔gpu", "grid↔term_handler")
3. Estimate scope: "N boundaries, ~M findings"
4. Order: leaks first (layer bleeding), then waste (perf), then exposure (encapsulation)

### Plan Format

```
## Implementation Hygiene Review: {target}

**Scope:** N boundaries reviewed, ~M findings (X leak, Y waste, Z exposure)

### {Boundary: Module A → Module B}

**Interface types:** {list types crossing this boundary}
**Entry points:** {list key functions}

1. **[LEAK]** `file:line` — {description}
2. **[WASTE]** `file:line` — {description}
3. **[EXPOSURE]** `file:line` — {description}
...

### {Next Boundary}
...

### Execution Order

1. Layer bleeding fixes (may require interface changes)
2. Error handling fixes (may add error variants)
3. Ownership/allocation fixes (perf, no API change)
4. Encapsulation fixes (minimize boundary types)
5. Run `./test-all.sh` to verify no behavior changes
6. Run `./clippy-all.sh` to verify no regressions
```

## Important Rules

1. **No architecture changes** — Don't propose new modules, new crates, or restructured dependency graphs
2. **No code style fixes** — Don't flag naming, comments, or file organization (that's `/code-hygiene-review`)
3. **Trace, don't grep** — Follow actual data flow through the code, don't just search for patterns
4. **Read both sides** — Always read both the producer and consumer of a boundary
5. **Understand before flagging** — Some apparent violations are intentional (e.g., `app.rs` coordinating between tabs and windows is orchestration, not layer bleeding)
6. **Be specific** — Every finding must have `file:line`, the boundary it violates, and a concrete fix
7. **Compare to reference terminals** — When in doubt, check how Alacritty/WezTerm/Ghostty handle the same boundary at `~/projects/reference_repos/console_repos/`
