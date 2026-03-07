---
section: "06"
title: "AI-Generated Release Notes"
status: complete
goal: "Both nightly and tagged releases get Claude-generated release notes from commit/PR context"
inspired_by:
  - "ori_lang auto-release.yml (Copilot SDK + Claude Sonnet for release notes)"
  - "ori_lang .claude/commands/publish-release.md (manual Claude-generated notes)"
depends_on: ["04", "05"]
sections:
  - id: "06.1"
    title: "Release note generation step"
    status: complete
  - id: "06.2"
    title: "Add to nightly workflow"
    status: complete
  - id: "06.3"
    title: "Add to release workflow"
    status: complete
  - id: "06.4"
    title: "Fallback strategy"
    status: complete
  - id: "06.5"
    title: "Completion Checklist"
    status: complete
---

# Section 06: AI-Generated Release Notes

**Status:** Complete
**Goal:** Both the nightly and tagged release workflows generate rich,
human-quality release notes using Claude via the GitHub Copilot SDK. The
notes describe what changed from the user's perspective, categorized by
type (features, bug fixes, improvements, internals).

**Context:** The current nightly body is a static template ("Rolling nightly
build from main"). The release workflow uses GitHub's `generate_release_notes`
which produces a raw commit list with no curation. ori_lang's `auto-release.yml`
demonstrates a proven pattern: gather commits + PR descriptions, send them to
Claude Sonnet via the Copilot SDK, and use the AI-generated output as the
release body. The result is dramatically better than commit lists.

**Reference implementations:**
- **ori_lang** `auto-release.yml:158-274`: Gathers commit log + merged PR
  bodies, sends to Claude Sonnet 4.6 via the Copilot SDK Python client,
  falls back to conventional-commit categorization if AI fails.
- **ori_lang** `.claude/commands/publish-release.md`: Same concept for
  manual releases via the Claude Code slash command.

**Depends on:** Sections 04 and 05 (the workflows must exist before adding
the release notes generation step to them).

---

## 06.1 Release note generation step

The release note generation is a reusable pattern: gather context, prompt
Claude, write to a file. Both workflows use the same logic with different
prompts (nightly is shorter/lighter, tagged releases are more detailed).

**Prerequisites (GitHub repo settings):**
- [ ] Create or reuse a GitHub App for release automation (like ori_lang's
  `ori-release-bot`), or use the built-in `GITHUB_TOKEN` if sufficient for
  PR listing.
- [ ] Add `COPILOT_GITHUB_TOKEN` secret to the repo (grants Copilot SDK
  access to Claude models).

**Shared pattern (used by both workflows):**

The steps are:

1. **Gather context:** Find previous release tag, get commit log and merged
   PR descriptions since that tag.
2. **Generate notes:** Send context to Claude Sonnet via Copilot SDK with
   a tailored prompt.
3. **Fallback:** If AI generation fails, categorize commits by conventional
   commit prefix.
4. **Write:** Output to `/tmp/release-notes.txt` for the release step to
   consume.

- [ ] Create the context-gathering step (reusable across both workflows):
  ```yaml
  - name: Gather release context
    id: commits
    env:
      GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    run: |
      # Find previous tag for release notes range
      PREV_TAG=$(git tag --sort=-creatordate | grep -v '^nightly$' \
        | grep '^v\|^nightly-' | head -1)

      if [[ -n "$PREV_TAG" ]]; then
        PREV_DATE=$(git log -1 --format=%aI "$PREV_TAG")
        LOG=$(git log "${PREV_TAG}..HEAD" \
          --pretty=format:"- %s (%h)" --no-merges)
      else
        LOG=$(git log --pretty=format:"- %s (%h)" --no-merges -20)
        PREV_DATE=""
      fi

      # Fetch merged PR descriptions — richest context available
      if [[ -n "$PREV_DATE" ]]; then
        PR_BODIES=$(gh pr list --state merged --base main --limit 20 \
          --json number,title,body,mergedAt \
          --jq "[.[] | select(.mergedAt >= \"$PREV_DATE\")] \
            | .[] | \"## PR #\\(.number): \\(.title)\\n\\(.body // \"(no description)\")\\n\"")
      else
        PR_BODIES=$(gh pr list --state merged --base main --limit 10 \
          --json number,title,body \
          --jq '.[] | "## PR #\(.number): \(.title)\n\(.body // "(no description)")\n"')
      fi

      echo "prev_tag=$PREV_TAG" >> $GITHUB_OUTPUT
      echo "$LOG" > /tmp/commit-log.txt
      echo "$PR_BODIES" > /tmp/pr-bodies.txt
  ```

  **Note:** This step needs `fetch-depth: 0` on the checkout to access tag
  history. Update the checkout step in both workflows accordingly.

---

## 06.2 Add to nightly workflow

**File(s):** `.github/workflows/nightly.yml`

Add AI release notes to the nightly release job. The nightly prompt is
concise — users want a quick summary, not a changelog.

- [ ] Update `actions/checkout@v4` in the release job to `fetch-depth: 0`
- [ ] Add Node.js and Python setup steps (required by Copilot SDK)
- [ ] Add the context-gathering step from 06.1
- [ ] Add the AI generation step with a nightly-specific prompt:
  ```yaml
  - name: Generate AI release notes
    id: ai_notes
    env:
      COPILOT_GITHUB_TOKEN: ${{ secrets.COPILOT_GITHUB_TOKEN }}
    run: |
      TAG="nightly"
      PREV_TAG="${{ steps.commits.outputs.prev_tag }}"

      python3 << 'PYEOF'
      import asyncio, sys
      from pathlib import Path

      COMMIT_LOG = Path("/tmp/commit-log.txt").read_text()
      PR_BODIES = Path("/tmp/pr-bodies.txt").read_text()

      PROMPT = f"""Write brief nightly release notes for **oriterm** (a GPU-accelerated terminal emulator). This is an automated nightly build, not a milestone release.

      Format: A short summary paragraph (2-3 sentences) followed by a bulleted list of changes. Group into sections only if there are 5+ changes; otherwise use a flat list. Omit empty sections.

      Sections (if needed): Features, Bug Fixes, Improvements

      Rules:
      - PR descriptions are your PRIMARY source
      - Write from the user's perspective
      - Past tense ("Added", "Fixed")
      - Skip CI/docs/chore commits
      - Keep it concise — this is a nightly, not a milestone
      - Do NOT wrap in markdown fences
      - If there are very few changes, a 1-2 sentence summary is fine

      Pull request descriptions:
      {PR_BODIES}

      Commit log:
      {COMMIT_LOG}"""

      async def main():
          from copilot import CopilotClient, PermissionHandler
          client = CopilotClient()
          await client.start()
          try:
              session = await client.create_session({
                  "model": "claude-sonnet-4.6",
                  "streaming": False,
                  "on_permission_request": PermissionHandler.approve_all,
              })
              done = asyncio.Event()
              result = []

              def on_event(event):
                  t = event.type.value if hasattr(event.type, 'value') else str(event.type)
                  if t == "assistant.message":
                      content = event.data.content if hasattr(event.data, 'content') else str(event.data)
                      result.append(content)
                  elif t == "session.idle":
                      done.set()

              session.on(on_event)
              await session.send({"prompt": PROMPT})
              await asyncio.wait_for(done.wait(), timeout=120)

              if result:
                  Path("/tmp/release-notes.txt").write_text(result[-1])
                  print("AI release notes generated successfully")
              else:
                  raise RuntimeError("No response from model")
          finally:
              await client.stop()

      try:
          asyncio.run(main())
      except Exception as e:
          print(f"::warning::AI release notes failed ({e}), falling back to commit log")
          log = Path("/tmp/commit-log.txt").read_text().strip()
          sections = {"Features": [], "Bug Fixes": [], "Other": []}
          for line in log.split("\n"):
              line = line.strip()
              if not line or not line.startswith("- "):
                  continue
              subject = line[2:]
              if subject.startswith("feat"):
                  sections["Features"].append(line)
              elif subject.startswith("fix"):
                  sections["Bug Fixes"].append(line)
              else:
                  sections["Other"].append(line)
          body = ""
          for section, items in sections.items():
              if items:
                  body += f"### {section}\n\n" + "\n".join(items) + "\n\n"
          if not body.strip():
              body = "### Changes\n\n" + log
          Path("/tmp/release-notes.txt").write_text(body.strip())
      PYEOF
  ```

- [ ] Update the `softprops/action-gh-release` step to use the generated
  notes file instead of the static body:
  ```yaml
  - name: Read release notes
    id: notes
    run: echo "body=$(cat /tmp/release-notes.txt)" >> $GITHUB_OUTPUT

  - name: Update nightly release
    uses: softprops/action-gh-release@v2
    with:
      tag_name: nightly
      name: ${{ steps.meta.outputs.name }}
      body_path: /tmp/release-notes.txt
      files: artifacts/*
      prerelease: true
      make_latest: false
    env:
      GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
  ```
  (Use `body_path` instead of inline `body` to avoid shell escaping issues.)

---

## 06.3 Add to release workflow

**File(s):** `.github/workflows/release.yml`

Add AI release notes to the tagged release workflow. The release prompt is
more detailed — these are milestone releases that people read carefully.

- [ ] Update `actions/checkout@v4` in the release job to `fetch-depth: 0`
- [ ] Add Node.js, Python, and Copilot SDK setup steps
- [ ] Add the context-gathering step from 06.1
- [ ] Add the AI generation step with a release-specific prompt:
  ```python
  PROMPT = f"""Write detailed release notes for **oriterm** ({TAG}), a
  GPU-accelerated terminal emulator in Rust. The audience is developers
  and terminal enthusiasts.

  Format:
  Start with a 1-2 sentence summary describing the theme of this release.

  Then group changes into sections (omit empty ones):
  - **Features** — new user-facing capabilities
  - **Bug Fixes** — corrected behavior
  - **Improvements** — enhancements, performance, error messages
  - **Internals** — architecture changes, code quality (include for alpha
    releases — development IS the product at this stage)

  For each bullet:
  - **Bold title** followed by 1-2 sentences explaining what changed and why
  - Use past tense ("Added", "Fixed", "Improved")
  - Reference affected areas (renderer, grid, PTY, key encoding, etc.)

  Rules:
  - PR descriptions are your PRIMARY source — they contain summaries of
    what changed and why
  - The commit log is supplementary
  - Every substantive change gets a meaningful description
  - Skip CI automation PRs
  - Do not reproduce test plan checklists
  - Do NOT wrap in markdown fences

  Pull request descriptions:
  {PR_BODIES}

  Commit log ({PREV_TAG}..{TAG}):
  {COMMIT_LOG}"""
  ```

- [ ] Replace `generate_release_notes: true` in the `softprops/action-gh-release`
  step with `body_path: /tmp/release-notes.txt`

---

## 06.4 Fallback strategy

The AI generation must never block a release. If the Copilot SDK fails
(token expired, rate limit, SDK bug), the fallback categorizes commits by
conventional commit prefix — functional but less polished.

- [ ] Both workflows use the same Python fallback block (shown in 06.2)
- [ ] The fallback runs inside the same `except Exception` handler
- [ ] The fallback writes to the same `/tmp/release-notes.txt` path
- [ ] The `::warning::` annotation surfaces in the GitHub Actions log so
  failures are visible but non-blocking
- [ ] If even the fallback fails (e.g., `/tmp/commit-log.txt` is empty),
  a final safety net writes a minimal body: "See commit history for changes."

---

## 06.5 Completion Checklist

- [ ] `COPILOT_GITHUB_TOKEN` secret added to the repository
- [ ] Nightly workflow generates AI release notes on each push to `main`
- [ ] Tagged release workflow generates AI release notes on `v*` tags
- [ ] Nightly notes are concise (summary + bullet list)
- [ ] Release notes are detailed (themed summary + categorized sections)
- [ ] Fallback produces conventional-commit-categorized notes when AI fails
- [ ] Both workflows use `fetch-depth: 0` in the release job checkout
- [ ] Both workflows install Node.js, Python, and the Copilot SDK
- [ ] `body_path` used (not inline `body`) to avoid shell escaping issues
- [ ] Release notes do NOT include markdown fences or raw commit hashes

**Exit Criteria:** Push a commit to `main`, verify the nightly release has
AI-generated notes. Tag a test release, verify the release has detailed
AI-generated notes. Disconnect the Copilot token, verify the fallback
produces categorized commit notes without failing the workflow.
