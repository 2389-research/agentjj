# ABOUTME: Post-mortem on why jj-lib was removed from agentjj
# ABOUTME: Documents the architectural mismatch between jj's working copy model and agent workflows

# Why We Removed jj from agentjj

## The Bet

agentjj was built on a thesis: Jujutsu (jj) has a better change model than git, so embedding jj-lib would give agents a better version control experience. jj offers an operation log (every action reversible), first-class conflicts, stable change IDs, and no staging area confusion.

The thesis was wrong — not because jj is bad, but because the problems jj solves are human problems, and agents have different problems.

## What Went Wrong

### 1. Commit Absorption

jj's working copy is always a commit. When agentjj "commits," it performs a squash operation — folding the working copy into its parent, then creating a new empty working copy on top. In practice, this meant sequential commits from subagents would absorb prior commits into a single squashed commit, destroying the intended git history.

From a field report during the muesli project:

> "The jj/agentjj issue in Task 2 was annoying — it absorbed multiple prior commits into a single squashed commit, which messed up the git history. Told subsequent subagents to use plain git instead."

The agent gave up on agentjj and fell back to git. When your own users route around your tool, that's the clearest signal possible.

### 2. Parallel Agent Bundling

jj has one working copy (`@`) per workspace. When multiple subagents work in the same repo simultaneously, all their changes land in the same working copy. When one agent commits, it grabs everything — its own changes and every other agent's in-progress work.

From the mammoth project:

> "Parallel subagent execution with agentjj can cause commit bundling — both agents' changes end up in one commit since jj auto-commits the working copy."

This isn't a bug in agentjj's implementation. It's jj's design. The working copy model is fundamentally single-writer.

Git doesn't have this problem because `git add` is explicit selection. An agent can `git add src/foo.rs && git commit` without touching any other agent's uncommitted work in `src/bar.rs`.

### 3. Colocated Mode Confusion

Running jj alongside git (colocated mode) created a persistent source of confusion. Files tracked by jj appeared as "deleted" in git's staging area. Git status output was misleading. Agents that fell back to git commands (which happened whenever agentjj failed) would see inconsistent state.

From field observations:

> "In jj colocated repos, git status output is misleading. Files tracked by jj appear as 'deleted' in git's staging area, and the disk files appear as 'untracked'."

### 4. Detached HEAD Syndrome

jj's commit operations sometimes failed to sync back to git, leaving HEAD detached. This required manual recovery (`git checkout main`) and meant the jj commit existed in jj's state but was invisible to git, CI, and other tools.

This was patched in v0.3.1 by importing git refs during colocated init, but it was symptomatic of the deeper colocation fragility.

### 5. The Codebase Was Already Voting

By v0.3.1, bug fixes had already migrated three operations away from jj to git:

- `diff` → `git diff` (jj CLI wasn't installed, `Command::new("jj")` failed)
- `orient` recent changes → `git log`
- `log_ascii` graph display → `git log --graph`

Each fix independently concluded that git was more reliable than jj for that operation. The codebase was migrating itself.

## What jj Solves vs. What Agents Need

| jj Feature | Human Problem It Solves | Agent Relevance |
|-----------|------------------------|----------------|
| No staging area | Humans forget to `git add` | Agents never forget. `git add` is explicit selection, which is a *feature* for agents. |
| First-class conflicts | Humans panic at merge conflicts | Agents rarely encounter conflicts (single-branch workflow). When they do, they resolve programmatically. |
| Operation log | Humans want to undo mistakes | Valuable, but reimplementable with git refs. |
| Change IDs | Humans lose track after rebase | Agents don't rebase. Commit SHAs are fine. |
| Immutable commits | Humans accidentally amend | Agents commit intentionally. |

The pattern: jj's innovations address human cognitive limitations (forgetting to stage, panicking at conflicts, losing track of commits). Agents don't have these limitations. They have different ones (parallel execution, explicit file selection, programmatic state management) where git is actually better.

## What We Kept

The valuable parts of agentjj had nothing to do with jj:

- **Code intelligence** (`symbol`, `context`, `affected`) — tree-sitter, not jj
- **Batch operations** (`bulk`) — orchestration layer, not jj
- **Structured output** (`--json`) — formatting, not jj
- **Repo orientation** (`orient`) — already migrated to git
- **Manifest system** — sidecar metadata in `.agent/`, not jj
- **Typed commits** — sidecar metadata in `.agent/changes/`, not jj

## What We Replaced

| jj Feature | Git Replacement |
|-----------|----------------|
| Checkpoints (operation log) | `refs/agentjj/checkpoints/<name>` — lightweight local refs |
| Undo to checkpoint | `git reset --hard refs/agentjj/checkpoints/<name>` |
| Undo N steps | `git reflog` + `git reset` |
| Graph (ASCII) | `git log --graph --oneline --decorate` |
| Graph (mermaid/dot) | Parse `git log --format` + custom generation |
| Commit | `git add <paths> && git commit -m "msg"` |
| Status | `git status --porcelain` + `git diff --name-status` |

## Lessons

1. **Solve agent problems, not human problems.** jj is a better human VCS. Agents need a better agent VCS. These are not the same thing.

2. **The staging area is a feature.** For humans, `git add` is friction. For agents, it's explicit selection — the ability to commit exactly the files you want without grabbing everything in the working directory.

3. **Colocation adds complexity without proportional value.** Maintaining two VCS states (`.git/` and `.jj/`) in sync is fragile. Every colocation bug required falling back to git anyway.

4. **Watch where your fixes drift.** When every bug fix moves an operation from system A to system B, that's data about which system is more reliable. Listen to it.

5. **Field reports over unit tests.** The commit absorption bug passed all 134 tests. It only surfaced when real agents did real multi-commit workflows in production. Journal entries from agents-in-the-field caught what test suites missed.
