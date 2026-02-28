---
name: continue-roadmap
description: Resume work on the ori_term rebuild roadmap, picking up where we left off
argument-hint: "[section]"
---

# Continue Roadmap

Resume work on the ori_term rebuild roadmap, picking up where we left off.

## Usage

```
/continue-roadmap [section]
```

- No args: Auto-detect first incomplete item sequentially (01 → 02 → ...)
- `section-5`, `5`, or `gpu`: Continue Section 5 (Window + GPU Rendering)
- Any section number or keyword: Use `plans/roadmap/index.md` to find sections by keyword

## Finding Sections by Topic

Use `plans/roadmap/index.md` to find sections by keyword. The index contains searchable keyword clusters for each section.

---

## Workflow

### Step 1: Run the Scanner

Run the roadmap scanner script to get current status:

```bash
.claude/skills/continue-roadmap/roadmap-scan.sh plans/roadmap
```

This outputs:
- One line per section: `[done]` or `[open]` with progress stats
- Detail block for the **first incomplete section**: subsection statuses (with blocked counts), first 5 **unblocked** items, blocker summary, and blocker chain

### Step 2: Determine Focus Section

**If argument provided**, find the matching section file and skip to Step 3.

**If no argument provided**, check the **Priority Queue** in `plans/roadmap/index.md` first. The first incomplete priority section becomes the focus. If all priority sections are complete (or the queue is empty), fall back to the scanner's `=== FOCUS ===` section — the first section with `[ ]` items, scanning sequentially from Section 01.

#### Dependency Skip Rule

Only skip a section if **all** of these are true:
1. The section has explicit dependencies listed in the Dependency DAG (see `plans/roadmap/index.md`)
2. One or more of those dependencies has `status: not-started` or `status: in-progress` (prerequisite isn't complete)
3. The incomplete work in the current section actually **requires** the blocker (not all items may be blocked)

If a section has some blocked items and some unblocked items, **work the unblocked items** rather than skipping.

#### Blocker References (2-Way)

When you discover a blocker, you **must** add a 2-way reference so both sides are linked:

1. **On the blocked item** — Add `<!-- blocked-by:X -->` where X is the blocker section number
2. **On the blocker item** — Add `<!-- unblocks:X.Y -->` where X.Y is the blocked subsection ID

**Tag format**: Machine-readable, no free text. Human-readable names come from frontmatter lookup.
- `<!-- blocked-by:18 -->` — blocked by Section 18
- `<!-- blocked-by:18 --><!-- blocked-by:3 -->` — blocked by multiple sections
- `<!-- unblocks:5.3 -->` — unblocks subsection 5.3

**Both references must be added at the same time.** A one-way reference is incomplete.

Example:
```markdown
## 16.3 Tab Bar Hit Testing
- [ ] Tab hover preview  <!-- blocked-by:7 -->

## In Section 7, subsection 7.9:
- [ ] Animation system  <!-- unblocks:16.3 -->
```

**Parent inheritance**: Nested `- [ ]` items (indented) inherit their parent's blocker. Only tag the top-level item.

This ensures:
- The scanner correctly counts blocked vs unblocked items
- When completing a blocker, you can `grep 'unblocks:'` to find what it unblocks
- When reviewing a blocked item, `grep 'blocked-by:'` shows what prerequisite is missing

### Step 2.5: Blocker Chain Resolution

When the scanner shows blocked items, analyze the blocker chain:

1. Read the **Blocker summary** and **Blocker chain** from scanner output
2. Classify each blocker:
   - **READY**: All its dependencies are `[complete]` — can start implementing now
   - **IN PROGRESS**: Section already being worked on — progress will eventually unblock
   - **WAITING**: Has incomplete dependencies — blocked itself, can't start yet
3. Build and present a blocker tree in the summary:
   ```
   Blocker Tree:
   ├─ Section 07: 2D UI Framework [not-started] — READY (deps satisfied: 06 [complete])
   │  └─ blocks 12 items here
   ├─ Section 03: Cross-Platform [in-progress, 40%] — IN PROGRESS
   │  └─ blocks 5 items here
   └─ Section 05: Window + GPU [not-started] — WAITING on Section 04
      └─ blocks 3 items here
   ```

### Step 3: Load Section Details

Read the focus section file at the line numbers reported by the scanner. Extract:

1. **Section title** from the `# Section N:` header
2. **Completion stats**: from scanner output
3. **First incomplete item**: The first `- [ ]` line and its context (subsection header, description)
4. **Recently completed items**: Last few `- [x]` items for context

### Step 4: Present Summary

Present to the user:

```
## Section N: [Name]

**Progress:** X/Y items complete (Z%)
**Actionable:** A unblocked, B blocked (by N sections)

### Recently Completed
- [last 2-3 completed items]

### Next Up (Unblocked)
**Subsection X.Y: [Subsection Name]**
- [ ] [First unblocked incomplete item]
  - [sub-items if any]

### Blockers
[Blocker tree from Step 2.5 — READY/IN PROGRESS/WAITING classification]

### Remaining in This Section
- [count of remaining unblocked items]
- [count of blocked items, with "blocked by N sections" note]
```

### Step 5: Ask What to Do

Use AskUserQuestion with options. The options depend on the blocker state:

**When there are unblocked items:**
1. **Start next task (Recommended)** — Begin implementing the first unblocked item
2. **Show task details** — See more context about the task (read spec, find related code)
3. **Pick different task** — Choose a specific unblocked task from this section
4. **Tackle a blocker** — Work on a READY blocker to unblock items (ranked by impact: most items unblocked first)
5. **Switch sections** — Work on a different section

**When ALL remaining items are blocked:**
1. **Tackle deepest ready blocker (Recommended)** — Work on the READY blocker that unblocks the most items
2. **Show blocker details** — See what the blocker requires and its dependency chain
3. **Switch sections** — Work on a different section

### Step 6: Execute Work

Based on user choice:
- **Start next task**: Begin implementing the first unblocked item, following the Implementation Guidelines below
- **Show task details**: Read relevant section content, explore codebase for implementation location, check reference repos
- **Pick different task**: List all unblocked incomplete items in the section, let user choose
- **Tackle a blocker**: Switch to the blocker section and begin implementing its first unchecked item. When the blocker is complete, return to update the blocked items.
- **Switch sections**: Ask which section to switch to

---

## Implementation Guidelines

### Scope Rule: ALL Checkboxes in the Section Are In Scope

**Every `- [ ]` checkbox within the current section is part of that section's work — no exceptions.** This includes:

- **Testing** checkboxes (unit tests, integration tests, visual regression tests)
- **Build verification** checkboxes (clippy, cross-compilation)
- **Platform-specific** checkboxes (Windows, Linux, macOS)
- Any other sub-item checkboxes nested under a parent item

**Do NOT defer items to other sections.** If subsection 5.13 has `[ ] Headless GPU integration tests`, that checkbox is part of Section 5 — not Section 23 (Performance). Individual sections track their own testing coverage.

**A subsection is only complete when ALL its checkboxes are checked.** Do not mark a subsection as complete or move to the next subsection while checkboxes remain unchecked.

### Verification Rule: Empty Checkboxes Must Be Verified

**Never check off a `[ ]` item without verifying it.** Before marking any item `[x]`:

1. **Read the relevant code** — confirm the feature/test actually exists
2. **Run the test** — if it's a test item, run it and confirm it passes
3. **Check the plan** — if it's an implementation item, verify behavior matches the plan

Checking off items without verification defeats the purpose of the roadmap.

### Before Writing Code

1. **Read the plan** — Understand exactly what the section requires
2. **Check reference repos** — Look in `~/projects/reference_repos/console_repos/` for established patterns (Alacritty, WezTerm, Ghostty, Ratatui, Crossterm)
3. **Read the old code** — Check `_old/src/` for the prototype implementation (reference only — don't copy patterns, just understand intent)
4. **Explore the codebase** — Use Explore agent to find where features should be implemented

### While Writing Code

1. **Follow existing patterns** — Match the style of surrounding code
2. **Follow CLAUDE.md coding standards** — Error handling, unsafe rules, linting, module organization, etc.
3. **Add tests** — Unit tests for `oriterm_core`, integration tests for `oriterm`
4. **Check off items** — Update section file checkboxes as you complete sub-items

### After Writing Code

1. **Run checks** — `./clippy-all.sh` and `./test-all.sh` to verify everything passes
2. **Build** — `./build-all.sh` to verify cross-compilation
3. **Update section file** — Check off completed items with `[x]`
4. **Update YAML frontmatter** — See "Updating Section File Frontmatter" below
5. **Commit with clear message** — Reference the section and task

---

## Updating Section File Frontmatter

Section files use YAML frontmatter for machine-readable status tracking. **You must keep this in sync** when completing tasks.

### Frontmatter Structure

```yaml
---
section: 5
title: Window + GPU Rendering
status: in-progress          # Section-level status
tier: 2
goal: Open a frameless window...
sections:
  - id: "5.1"
    title: Render Pipeline Architecture
    status: complete         # Subsection-level status
  - id: "5.2"
    title: winit Window Creation
    status: in-progress
---
```

### Status Values

- `not-started` — No checkboxes completed in subsection/section
- `in-progress` — Some checkboxes completed, some pending
- `complete` — All checkboxes completed

### When to Update

**After completing task checkboxes**, update the frontmatter:

1. **Update subsection status** based on checkboxes under that `## X.Y` header:
   - All `[x]` → `status: complete`
   - Mix of `[x]` and `[ ]` → `status: in-progress`
   - All `[ ]` → `status: not-started`

2. **Update section status** based on subsection statuses:
   - All subsections complete → `status: complete`
   - Any subsection in-progress → `status: in-progress`
   - All subsections not-started → `status: not-started`

---

## Verification/Audit Workflow

When auditing roadmap accuracy (verifying status rather than implementing features), follow this workflow:

### Step 1: Compare Frontmatter to Body

Before testing anything, check if frontmatter matches checkbox state:

1. Read the YAML frontmatter subsection statuses
2. Scan the body for `[x]` and `[ ]` checkboxes under each `## X.Y` header
3. **If they don't match** — the roadmap is stale and needs updating

### Step 2: Test Claimed Status

Don't trust checkboxes blindly. Verify actual implementation:

1. **For `[x]` items**: Write quick test to confirm feature works
2. **For `[ ]` items**: Write quick test to confirm feature fails/is missing
3. **Document discrepancies**: Note items where claimed status doesn't match reality

### Step 3: Update Body Checkboxes

Fix checkboxes to match verified reality:

- Feature works → `[x]`
- Feature broken/missing → `[ ]`

### Step 4: Update Frontmatter Immediately

**Never leave frontmatter stale.** After updating body checkboxes:

1. Recalculate each subsection status from its checkboxes
2. Update subsection `status` values in frontmatter
3. Recalculate section status from subsection statuses
4. Update section `status` value in frontmatter

---

## Checklist

When completing a roadmap item:

- [ ] Read plan section thoroughly
- [ ] Check reference repos for established patterns
- [ ] Implement feature
- [ ] Add unit tests (oriterm_core) and/or integration tests (oriterm)
- [ ] Run `./clippy-all.sh` — no warnings
- [ ] Run `./test-all.sh` — all tests pass
- [ ] Run `./build-all.sh` — cross-compilation succeeds
- [ ] Update section file:
  - [ ] Check off completed items with `[x]`
  - [ ] Update subsection `status` in YAML frontmatter if subsection is now complete
  - [ ] Update section `status` in YAML frontmatter if all subsections are now complete
- [ ] Commit with section reference in message

---

## Maintaining the Roadmap Index

**IMPORTANT:** When adding new items to the roadmap, update `plans/roadmap/index.md`:

1. **Adding items to existing section**: Add relevant keywords to that section's keyword cluster
2. **Creating a new section**: Add a new keyword cluster block and table entry
3. **Removing/renaming sections**: Update the corresponding entries

The index enables quick topic-based navigation. Keep keyword clusters concise and include both formal names and common aliases developers might search for.
