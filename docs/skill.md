---
name: agentjj
description: Agent-first version control with jj. Self-contained (no jj install needed). Auto-colocates with git repos. Complete orientation, typed commits, symbol queries, checkpoints, undo, bulk operations, impact analysis. Use --json for machine output. Start with 'agentjj orient' in any repo.
---

# agentjj - Agent-First Version Control

An agent-oriented porcelain for Jujutsu (jj) version control. Designed to make AI agents love version control.

**Zero-install**: agentjj embeds jj-lib directly - no separate jj installation required.

**Git-compatible**: Automatically colocates with existing git repos. Git continues to work normally.

## Quick Start

```bash
# In any git repo - agentjj auto-initializes jj colocated
agentjj orient                  # Get complete repo orientation

# Optional: create agent manifest for permissions/invariants
agentjj init                    # Create .agent/manifest.toml
```

## Core Philosophy

1. **Everything is JSON** - Use `--json` for machine-parseable output
2. **Self-documenting** - `agentjj schema` shows all output formats
3. **Safe by default** - Checkpoints and undo for easy recovery
4. **Batch-friendly** - Bulk operations for efficiency
5. **Context-rich** - Get exactly what you need without bloat

## Essential Commands

### Orient (Start Here)

```bash
agentjj orient                  # Complete repo briefing
agentjj --json orient           # As structured JSON
```

Returns: current state, codebase stats, recent changes, capabilities, quick start guide.

### Status & Discovery

```bash
agentjj status                  # Current change, operation, files
agentjj suggest                 # What should I do next?
agentjj validate                # Are my changes ready to push?
```

### Reading Code

```bash
agentjj read src/main.rs                    # Read file
agentjj symbol src/api.py                   # List all symbols
agentjj symbol src/api.py::process          # Get specific symbol
agentjj context src/api.py::process         # Minimal context to use symbol
agentjj affected src/api.py::process        # Impact analysis
```

### Bulk Operations (10x Efficiency)

```bash
agentjj bulk read src/a.rs src/b.rs src/c.rs
agentjj bulk symbols "src/**/*.rs"
agentjj bulk symbols "src/**/*.rs" --public-only
agentjj bulk context src/a.rs::foo src/b.rs::bar
```

### Files & Structure

```bash
agentjj files                               # List all files
agentjj files --pattern "src/**/*.rs"       # Filter by pattern
agentjj files --pattern "*.py" --symbols    # Include symbol counts
```

### Checkpoints & Recovery

```bash
agentjj checkpoint before-refactor          # Create checkpoint
agentjj checkpoint wip -d "work in progress"
agentjj undo                                # Undo last operation
agentjj undo --steps 3                      # Undo 3 operations
agentjj undo --to before-refactor           # Restore to named checkpoint
agentjj undo --dry-run                      # Preview what would be undone
```

### Committing Changes

```bash
agentjj commit -m "feat: add auth endpoint"                     # Basic commit
agentjj commit -m "fix: null check" --type behavioral           # Typed commit
agentjj commit -m "refactor: extract parser" --type refactor    # Refactor type
```

Types: `behavioral`, `refactor`, `schema`, `docs`, `deps`, `config`, `test`

### Changes & Diffs

```bash
agentjj diff                                # Show current diff
agentjj diff --against @                    # Working copy changes
agentjj diff --explain                      # With semantic summary
agentjj diff --against @--                  # Compare to 2 changes ago
```

### Typed Changes

```bash
agentjj change set -i "Add auth" -t behavioral -c feature
agentjj change list
agentjj change show <change_id>
```

Types: `behavioral`, `refactor`, `schema`, `docs`, `deps`, `config`, `test`
Categories: `feature`, `fix`, `perf`, `security`, `breaking`, `deprecation`, `chore`

### Apply & Push

```bash
agentjj apply \
  --intent "Fix null check" \
  --type behavioral \
  --category fix \
  --patch fix.patch

agentjj push                               # Push to remote
agentjj push --pr --title "Fix bug"        # Create PR
```

### Self-Documentation

```bash
agentjj schema                             # List all output schemas
agentjj schema --type context              # Show specific schema
agentjj schema --type orient               # See orient output format
agentjj skill                              # Full skill documentation
agentjj quickstart                         # Concise getting-started guide
```

## JSON Mode

**Always use `--json` for programmatic access:**

```bash
agentjj --json status
agentjj --json orient
agentjj --json context src/main.rs::main
agentjj --json bulk read file1.rs file2.rs
agentjj --json affected src/api.rs::handler
```

Errors also return JSON:
```json
{"error": true, "message": "Symbol path must be path/to/file::symbol_name"}
```

Exit codes: 0 = success, 1 = error

## Workflow Example

```bash
# 1. Orient yourself
agentjj orient

# 2. Create a checkpoint before making changes
agentjj checkpoint before-auth-refactor

# 3. Make code changes, then review
agentjj status                              # See what changed
agentjj diff                                # Review the diff

# 4. Commit
agentjj commit -m "feat: add OAuth login support"

# 5. Create another checkpoint, continue working
agentjj checkpoint after-auth

# 6. Push when ready
agentjj push --branch main
```

## Command Reference

| Command | Description |
|---------|-------------|
| `orient` | Complete repo orientation |
| `status` | Current state |
| `suggest` | Recommended next actions |
| `validate` | Check changes are ready |
| `read <path>` | Read file content |
| `symbol <path>` | Query symbols |
| `context <path>::<name>` | Get symbol context |
| `affected <path>::<name>` | Impact analysis |
| `bulk read <paths...>` | Read multiple files |
| `bulk symbols <pattern>` | Query symbols across files |
| `bulk context <symbols...>` | Get multiple contexts |
| `files [--pattern] [--symbols]` | List files |
| `commit -m "msg"` | Commit changes (most-used command) |
| `checkpoint <name>` | Create restore point |
| `undo [--steps N]` | Revert operations |
| `diff [--explain]` | Show changes |
| `change set/list/show` | Typed change metadata |
| `apply` | Apply intent transaction |
| `push [--pr]` | Push and optionally create PR |
| `schema [--type]` | Output schemas |
| `skill` | Full skill documentation |
| `quickstart` | Getting-started guide |
| `init` | Initialize agentjj |
| `manifest show/validate` | Manage manifest |

All commands support `--json` for structured output.

## Supported Languages

Symbol extraction: Python, Rust, JavaScript, TypeScript

## Pro Tips

1. **Start with `orient`** - It tells you everything about the repo
2. **Use `--json` always** - Reliable parsing, never breaks
3. **Checkpoint before risky changes** - Easy recovery
4. **Use `bulk` for efficiency** - One call, many results
5. **Check `affected` before changing** - Know the blast radius
6. **`suggest` when stuck** - Let the tool guide you
7. **`validate` before pushing** - Catch issues early
8. **`skill` for full docs** - The binary is the documentation
9. **`quickstart` for onboarding** - 6 steps to productivity
