# ABOUTME: Field observations from real Claude sessions using agentjj, with prioritized improvement recommendations
# ABOUTME: Data-driven analysis of 7 projects, 100+ agentjj interactions, and 17,000+ session turns

# agentjj Field Observations: What Real Agent Sessions Reveal

This document analyzes how agentjj performs in the wild. Every finding comes from
actual Claude Code sessions archived in ccvault, covering 7 projects, 100+ agentjj
interactions, and over 17,000 conversation turns. No speculation — just what the
data shows.

## Data Sources

| Project | Session ID | Model | Turns | Token Usage | Primary agentjj Use |
|---------|------------|-------|-------|-------------|---------------------|
| mammoth/dev | `e4204ed6` | Opus 4.6 | 5,224 | 77K | Commits, graph, status |
| hermit | `bf3bc382` | Opus 4.6 | 4,222 | 24K | Checkpoints (heavy) |
| mammoth/dot/editor | `41f61f88` | Opus 4.6 | 1,993 | 30K | 8+ sequential commits |
| blogwriter | `19e8fc41` | Opus 4.6 | 967 | 16K | Commits, noticed .jj issue |
| gorp/rs | `fe1b81da` | Opus 4.6 | 376 | 3K | Commit design docs |
| dotfiles | `8e00b72a` | Opus 4.5 | 742 | 3K | Failed — fell back to git |
| srcneed (building agentjj) | `7b67f133` | Opus 4.5 | 4,465 | 16K | Self-development |

---

## Finding 1: Documentation Lies to Every Agent (Critical)

### The Problem

The global `~/.claude/docs/source-control.md` file is loaded into every agent's
context. It contains **three commands that don't exist**:

```bash
# What agents are told to run:
agentjj describe -m "feat: add new endpoint"   # DOES NOT EXIST
agentjj affected                                # REQUIRES <SYMBOL> ARGUMENT
agentjj recover                                 # DOES NOT EXIST
```

```bash
# What actually works:
agentjj commit -m "feat: add new endpoint"      # CORRECT
agentjj affected src/api.py::handler             # CORRECT (needs path::symbol)
agentjj undo --to <checkpoint-name>              # CORRECT
```

### Impact: Measured

This error was hit in **3 separate sessions** across 3 different projects:

1. **mammoth/dev** (`e4204ed6`, turn `3aee2731`): Agent tries `agentjj describe`,
   gets error, runs `--help`, discovers `commit`. Cost: 3 extra tool calls.

2. **mammoth/dot/editor** (`41f61f88`, turn `eceaaf86`): Same sequence.
   Cost: 3 extra tool calls.

3. **gorp/rs** (`fe1b81da`, turn `e4564f60`): Same sequence.
   Cost: 3 extra tool calls.

Every time, the recovery pattern is identical:
```
describe → error → --help → discover commit → commit succeeds
```

The `affected` (no args) error was hit in 2 sessions:
```
affected → "required arguments not provided: <SYMBOL>" → confusion
```

### Cost Per Session

Each `describe` failure burns:
- 1 failed Bash call (`describe`)
- 1 Bash call (`--help`)
- 1 successful Bash call (`commit`)

That's **2 wasted tool calls per session**, multiplied across every project that
uses agentjj. With subagent-driven development dispatching 5-10 agents per session,
this multiplies fast.

### Fix

Update `~/.claude/docs/source-control.md`:

```bash
# Before (wrong)
agentjj describe -m "feat: add new endpoint"
agentjj affected
agentjj recover

# After (correct)
agentjj commit -m "feat: add new endpoint"
agentjj affected <path>::<symbol>
agentjj undo --to <checkpoint-name>
```

Also update the agentjj skill (`~/.claude/skills/agentjj/SKILL.md`) command
reference table — it currently doesn't mention `commit` at all in the command
table, despite `commit` being the most-used command in every session.

### Priority: P0 — Fix Today

This is a documentation bug, not a code bug. It costs nothing to fix and
eliminates waste in every future session.

---

## Finding 2: The .jj Directory Leaks Into Git (High)

### The Problem

When agentjj auto-colocates with a git repo, it creates a `.jj/` directory.
This directory appears in `git status` as untracked, and in at least 2 sessions,
`.jj/` internal files got committed to git accidentally.

### Evidence

**blogwriter** (`19e8fc41`, turn `d2d3cf52`):
> "Everything is committed except the .jj directory (which is from agentjj and
> should likely be in .gitignore)"

**mammoth/dev** (`e4204ed6`, turn `d5bcbb66`):
> "The `.jj/client/` directory files got committed — this is from agentjj's
> internal tracking"

**mammoth/dev** (context summary, turn `5afe29a4`):
> "First agentjj commit picked up entire repo"

### Root Cause

agentjj creates `.agent/.gitignore` to protect `.agent/checkpoints/` and
`.agent/changes/`, but it does **not** add `.jj/` to the repo's `.gitignore`
during auto-colocate. The `.jj/` directory is jj's internal state — equivalent
to `.git/` — and should never be tracked by git.

### Fix Options

**Option A (recommended)**: During auto-colocate, automatically append `.jj/` to
the repo's `.gitignore` if not already present. This is what the srcneed session
(`7b67f133`) implemented for `.agent/` subdirectories.

**Option B**: Add `.jj/` to the global gitignore (`~/.gitignore_global`). Less
invasive but requires user setup.

**Option C**: Document it as a known requirement. Weakest option — agents keep
forgetting.

### Priority: P0

This causes data pollution in git repos. Every agent that uses agentjj on a new
repo will hit this.

---

## Finding 3: First Commit Sweeps Extra Files (High)

### The Problem

In multiple sessions, the first `agentjj commit` in a repo picked up files that
weren't intended to be committed — including `.jj/client/` tracking files and
other unrelated changes.

### Evidence

**mammoth/dev** session summary:
> "First agentjj commit picked up entire repo"

**mammoth/dev** (`e4204ed6`, turn `b5679912`):
> "The `.jj/client/` directory already has many files — these seem to be from
> agentjj's change tracking. The stray file from this commit isn't really a
> concern since the directory..."

### Root Cause

jj's working copy model differs from git's. In jj, the working copy is
automatically snapshotted. When agentjj commits, it commits the entire working
copy state, not a staged subset. There's no equivalent of `git add <specific-file>`
followed by `git commit`.

### What Agents Actually Need

Looking at the session data, the commit pattern agents use is always:
```bash
agentjj commit -m "feat: add lexer"
```

They never selectively stage. They rely on committing everything and trusting
that the working directory only contains what they changed. This mostly works
but breaks when:

1. The repo has untracked files from other tools
2. Multiple subagents are working concurrently
3. The `.jj/` directory itself isn't gitignored

### Fix Options

**Option A**: Add `--paths` flag to `agentjj commit` for selective commits:
```bash
agentjj commit -m "feat: add lexer" --paths src/lexer.rs src/lexer_test.rs
```

**Option B**: Add `--exclude` flag to filter out unwanted paths:
```bash
agentjj commit -m "feat: add lexer" --exclude ".jj/**"
```

**Option C**: Automatically exclude `.jj/` and `.agent/` from commits.

Recommendation: **Option A + Option C**. Give agents explicit control while also
preventing the common case.

### Priority: P1

This causes messy git history. Not data-losing, but undermines trust in the tool.

---

## Finding 4: No Go Language Support (Medium-High)

### The Problem

agentjj supports symbol extraction for Python, Rust, JavaScript, and TypeScript
via tree-sitter. It does **not** support Go.

### Why This Matters

Three of the seven projects analyzed are Go codebases:
- **mammoth** (all variants) — Go
- **hermit** — Rust core + Go services
- **dot/editor** — Go

These are the heaviest agentjj users. The `symbol`, `context`, `affected`, and
`bulk symbols` commands are all useless on these projects.

### Evidence

In the mammoth/dev session (`e4204ed6`), agents never use `agentjj symbol` or
`agentjj context` — they use `Read` and `Grep` tools directly. The code
intelligence features are bypassed entirely.

### Fix

Add `tree-sitter-go` to Cargo.toml and add Go queries to `src/symbols.rs`.
The tree-sitter-go crate exists and is mature.

```toml
tree-sitter-go = "0.23"
```

Go is a particularly good fit for tree-sitter symbol extraction because of its
simple, regular syntax — functions, methods, structs, interfaces, and type
definitions are all straightforward to query.

### Priority: P1

The projects that use agentjj most can't use its code intelligence features.

---

## Finding 5: Dotfiles / Symlink Edge Case (Medium)

### The Problem

agentjj fails silently on repos with symlinked working directories.

### Evidence

**dotfiles** session (`8e00b72a`, turn `f3e9b248`):
> "Interesting, agentjj shows no files changed. This might be because the
> dotfiles repo has two working directories (the symlinked one at `/Users/harper/`
> and the actual repo at `/Users/harper/Public/src/personal/dotfiles`)"

The agent ran `agentjj status --json` and got zero changes. `git status` showed
modified files. The agent had to fall back to git entirely.

### Root Cause

jj's working copy tracking uses filesystem paths. When the working directory is
accessed via a symlink, the resolved path doesn't match what jj expects. The
colocated jj repo thinks nothing has changed.

### Fix Options

**Option A**: Resolve symlinks before initializing the jj workspace. Use
`std::fs::canonicalize()` on the repo path during auto-colocate.

**Option B**: Detect the mismatch and warn:
```json
{"warning": "Working directory appears symlinked. agentjj may not detect changes. Use the canonical path: /Users/harper/Public/src/personal/dotfiles"}
```

**Option C**: Document as known limitation.

Recommendation: **Option A** if feasible, **Option B** as fallback.

### Priority: P2

This is an edge case, but it causes total failure with no error message. Silent
failures are the worst kind.

---

## Finding 6: Checkpoints Are the Killer Feature (Positive)

### The Pattern

The hermit session (`bf3bc382`) used checkpoints as **phase gates** in a
multi-phase build. This is the most sophisticated agentjj usage observed:

```bash
agentjj checkpoint phase-8a-hermit-ffi
agentjj checkpoint phase-8b-ios-app
agentjj checkpoint review-fixes-round-2
agentjj checkpoint review-fixes-round-3
agentjj checkpoint integration-uniffi-ios
agentjj checkpoint integration-firebase-ios
agentjj checkpoint integration-persistence
agentjj checkpoint integration-complete
```

The agent created a checkpoint after each major milestone, enabling rollback if
later phases broke earlier work. This is exactly the workflow that makes jj
valuable for agents.

### What's Working

- Named checkpoints with descriptive names
- `agentjj undo --to <name>` for targeted rollback
- Checkpoint creation is fast and cheap
- Agents naturally adopt the pattern when they see it in docs

### What Could Be Better

**Checkpoint listing**: There's no `agentjj checkpoint list` command. Agents
can't see what checkpoints exist without checking `agentjj graph`.

**Checkpoint metadata**: Checkpoints don't carry descriptions unless you use
`-d`. A `--json` list of checkpoints with timestamps and descriptions would help
agents decide where to roll back to.

**Checkpoint in subagents**: Subagents create checkpoints that the parent agent
doesn't know about. A mechanism to query checkpoints created by subagents would
improve the multi-agent workflow.

### Recommendation

1. Add `agentjj checkpoint list [--json]`
2. Add timestamps and descriptions to checkpoint listing
3. Promote checkpoint usage more prominently in the skill docs — it's currently
   buried under "Checkpoints & Recovery" when it should be front-and-center

### Priority: P2 (enhancement, not a bug)

---

## Finding 7: Subagent Coordination Is Fragile (Medium)

### The Problem

When subagent-driven development dispatches multiple agents to work on the same
repo, their agentjj operations can interfere with each other.

### Evidence

**mammoth/dev** (`e4204ed6`, turn `68bf7333`):
> "Tasks 5.6 and 5.7's changes got bundled into the same commit via agentjj"

**mammoth/dev** session summary:
> "agentjj `describe` command doesn't exist - agents used `commit` instead"
> "First agentjj commit picked up entire repo files"

When multiple subagents run `agentjj commit`, they race to commit the working
copy. Since jj auto-snapshots the working copy, Agent A's commit might include
Agent B's uncommitted changes.

### What Agents Actually Do

In the mammoth/dev session, the orchestrator dispatched 7+ subagents in parallel.
Each subagent:
1. Made code changes
2. Ran tests
3. Committed via `agentjj commit -m "..."`
4. Reported back

When agents overlapped, their changes merged into single commits instead of
being isolated.

### Fix Options

**Option A (heavyweight)**: jj supports multiple working copies via workspaces.
`agentjj workspace create agent-1` would give each subagent its own working copy.
But this adds complexity.

**Option B (pragmatic)**: Add `--paths` to `agentjj commit` (see Finding 3).
Each subagent commits only the files it touched. Requires agents to track their
own file modifications.

**Option C (coordination)**: Provide a locking mechanism. `agentjj lock` before
changes, `agentjj unlock` after commit. Prevents concurrent modifications.

Recommendation: **Option B** first, evaluate **Option A** if the problem persists.

### Priority: P2

This only matters in subagent-driven development, but that's the dominant
workflow for complex projects.

---

## Finding 8: The `commit` Command Isn't in the Skill Docs (Medium)

### The Problem

The agentjj skill (`~/.claude/skills/agentjj/SKILL.md`) and docs/skill.md have a
comprehensive command reference table. The `commit` command is **not listed**.

Here's the command reference table from the skill:

```
| Command | Description |
|---------|-------------|
| orient | Complete repo orientation |
| status | Current state |
| ...    | ...          |
| push [--pr] | Push and optionally create PR |
```

The `commit` command — the single most-used agentjj command across all sessions —
is absent from the reference table.

### Why Agents Still Find It

They find it via:
1. CLAUDE.md has `agentjj commit -m "msg"` in the quick reference
2. `agentjj --help` lists all commands
3. After the `describe` failure, agents discover `commit` via `--help`

### Fix

Add `commit` to the command reference in both:
- `docs/skill.md`
- `~/.claude/skills/agentjj/SKILL.md`
- The `Workflow Example` section should use `commit`, not `change set` as the
  commit step

### Priority: P1

The most-used command isn't documented in the primary reference.

---

## Finding 9: `graph` Command Should Show More Context (Low)

### The Pattern

The mammoth/dev session used `agentjj graph` to verify commit history. The
output was useful but agents occasionally needed to cross-reference with
`git log` to get commit hashes and author information.

### Recommendation

Ensure `agentjj graph` output includes:
- Full commit hash (for cross-referencing with git)
- Timestamp
- Author (important when multiple agents commit)

### Priority: P3

---

## Summary: Prioritized Action Items

### P0 — Fix Today
| # | Finding | Type | Effort |
|---|---------|------|--------|
| 1 | Fix `source-control.md`: `describe` → `commit`, `affected` needs args, `recover` → `undo` | Doc fix | 5 min |
| 2 | Auto-add `.jj/` to `.gitignore` during colocate | Code fix | 30 min |

### P1 — Fix This Week
| # | Finding | Type | Effort |
|---|---------|------|--------|
| 3 | Add `commit` to skill docs command reference | Doc fix | 10 min |
| 4 | Add `--paths` flag to `agentjj commit` | Feature | 2-4 hrs |
| 5 | Add Go language support (tree-sitter-go) | Feature | 2-4 hrs |

### P2 — Fix This Sprint
| # | Finding | Type | Effort |
|---|---------|------|--------|
| 6 | Add `agentjj checkpoint list [--json]` | Feature | 1-2 hrs |
| 7 | Resolve symlinks during auto-colocate | Bug fix | 1-2 hrs |
| 8 | Subagent coordination (evaluate workspace approach) | Design | 4-8 hrs |

### P3 — Nice to Have
| # | Finding | Type | Effort |
|---|---------|------|--------|
| 9 | Richer `graph` output (hashes, timestamps, authors) | Enhancement | 1-2 hrs |

---

## Appendix: Command Usage Frequency (from sessions)

Based on actual tool calls across all analyzed sessions:

| Command | Frequency | Context |
|---------|-----------|---------|
| `agentjj commit -m "..."` | Very High | Used for all commits, conventional format |
| `agentjj orient` | High | Every session start, always first command |
| `agentjj checkpoint <name>` | High | Heavy in hermit; named restore points per phase |
| `agentjj status` | High | Before commits, after changes |
| `agentjj diff` | Moderate | Review before commit |
| `agentjj --help` | Moderate (3+) | Always after `describe` failure |
| `agentjj graph` | Low | Check revision history |
| `agentjj push` | Low | Only hermit and srcneed |
| `agentjj symbol` | None observed | Not used (Go projects lack support) |
| `agentjj context` | None observed | Not used (Go projects lack support) |
| `agentjj affected` | Error only | Always fails (wrong args from docs) |
| `agentjj bulk *` | None observed | Never used in analyzed sessions |
| `agentjj validate` | None observed | Never used in analyzed sessions |
| `agentjj suggest` | None observed | Never used in analyzed sessions |
| `agentjj change *` | None observed | Never used in analyzed sessions |
| `agentjj apply` | None observed | Never used in analyzed sessions |
| `agentjj schema` | None observed | Never used in analyzed sessions |

### What This Tells Us

The actual agent workflow is:
```
orient → [write code] → status → diff → commit → [repeat] → push
```

With checkpoints layered in for safety:
```
orient → checkpoint → [work] → commit → checkpoint → [work] → commit → push
```

The code intelligence features (`symbol`, `context`, `affected`, `bulk`) are
**entirely unused** in the analyzed sessions. This is partly because the Go
projects can't use them, and partly because agents already have Claude Code's
built-in `Read`, `Grep`, and `Glob` tools which serve the same purpose.

The governance features (`validate`, `suggest`, `change`, `apply`, `manifest`)
are **entirely unused**. Agents don't set up manifests or use typed changes —
they commit with `-m` and move on.

This suggests two possible conclusions:
1. These features need better discoverability and documentation, or
2. These features solve problems agents don't actually have

The data leans toward option 2 for governance features. Agents are effective
with the core workflow: orient, checkpoint, commit, push. The value of agentjj
is in making that core workflow reliable and safe — not in adding governance
layers that agents ignore.

---

## Appendix: Session Transcript Excerpts

### The `describe` → `commit` Recovery Pattern

Every session follows the same sequence:

```
Agent:   agentjj describe -m "feat: add DOT lexer and parser"
System:  Exit code 2
         error: unrecognized subcommand 'describe'
         Usage: agentjj [OPTIONS] <COMMAND>
         For more information, try '--help'.

Agent:   agentjj --help 2>&1 | head -30
System:  [lists all commands including 'commit']

Agent:   agentjj commit -m "feat: add DOT lexer and parser"
System:  [success]
```

### The dotfiles Failure

```
Agent:   agentjj status --json
System:  {"files": []}

Agent:   [thinking] "Interesting, agentjj shows no files changed. This might
         be because the dotfiles repo has two working directories..."

Agent:   git status
System:  [shows modified files]

Agent:   [thinking] "Git shows the modified file. The agentjj might not have
         picked it up because it wasn't initialized properly in this repo..."

Agent:   [falls back to git for remainder of session]
```

### The Checkpoint Phase-Gate Pattern (hermit)

```
Agent:   agentjj checkpoint phase-8a-hermit-ffi
System:  checkpoint 'phase-8a-hermit-ffi' created
         change: 6e6cd55db9a4
         restore with: agentjj undo --to phase-8a-hermit-ffi

[Agent completes Phase 8a work]

Agent:   agentjj checkpoint review-fixes-round-2
System:  checkpoint 'review-fixes-round-2' created
         change: 6e6cd55db9a4
         restore with: agentjj undo --to review-fixes-round-2

[Agent dispatches code reviewer subagent]
[Reviewer finds issues]
[Agent fixes issues]

Agent:   agentjj checkpoint review-fixes-round-3
System:  checkpoint 'review-fixes-round-3' created

[Pattern continues through 7 more checkpoints]
```
