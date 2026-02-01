# agentjj

Agent-first version control. A porcelain for [Jujutsu (jj)](https://github.com/martinvonz/jj) designed to make AI agents love source control.

**Zero-install**: Embeds jj-lib directly—no separate jj installation required.

**Git-compatible**: Auto-colocates with existing git repos. Git continues to work normally.

## Installation

### Homebrew (macOS/Linux)

```bash
brew install 2389-research/tap/agentjj
```

### Cargo

```bash
cargo install agentjj
```

### Binary releases

Download from [GitHub Releases](https://github.com/2389-research/agentjj/releases).

## Quick Start

```bash
# In any git repo—agentjj auto-initializes jj
agentjj orient                  # Complete repo orientation
agentjj init                    # Create .agent/manifest.toml (optional)

# Work with code
agentjj status                  # Current state
agentjj symbol src/main.rs      # List symbols
agentjj context src/main.rs::main  # Get symbol context

# Safe changes
agentjj checkpoint before-refactor  # Create restore point
agentjj validate                    # Check changes are ready
agentjj undo --to before-refactor   # Restore if needed
```

## Why agentjj?

Traditional VCS tools are designed for humans. Agents need:

| Human VCS | Agent VCS (agentjj) |
|-----------|---------------------|
| Interactive prompts | JSON-first output |
| Visual diffs | Structured change data |
| Manual commits | Typed changes with intent |
| Hope you remember | Checkpoints + undo |
| One file at a time | Bulk operations |
| "It worked on my machine" | Preconditions + validation |

## Commands

### Orientation & Status

```bash
agentjj orient              # Complete repo briefing (start here)
agentjj status              # Current change, files, typed metadata
agentjj suggest             # Recommended next actions
agentjj validate            # Check changes are ready to push
```

### Code Intelligence

```bash
agentjj read src/main.rs                    # Read file content
agentjj symbol src/api.py                   # List all symbols
agentjj symbol src/api.py::process          # Get specific symbol
agentjj context src/api.py::process         # Minimal context to use symbol
agentjj affected src/api.py::process        # Impact analysis
```

### Bulk Operations

```bash
agentjj bulk read src/a.rs src/b.rs src/c.rs
agentjj bulk symbols "src/**/*.rs"
agentjj bulk symbols "src/**/*.rs" --public-only
agentjj bulk context src/a.rs::foo src/b.rs::bar
```

### Checkpoints & Recovery

```bash
agentjj checkpoint before-refactor          # Create checkpoint
agentjj checkpoint wip -d "work in progress"
agentjj undo                                # Undo last operation
agentjj undo --steps 3                      # Undo 3 operations
agentjj undo --to before-refactor           # Restore to checkpoint
agentjj undo --dry-run                      # Preview what would be undone
```

### DAG Visualization

```bash
agentjj graph                    # ASCII DAG (native jj output)
agentjj graph --format mermaid   # Mermaid flowchart
agentjj graph --format dot       # Graphviz DOT
agentjj graph --limit 20         # Show more commits
agentjj graph --all              # All branches
```

### Typed Changes

```bash
agentjj change set -i "Add auth" -t behavioral -c feature
agentjj change list
agentjj change show <change_id>
```

**Types**: `behavioral`, `refactor`, `schema`, `docs`, `deps`, `config`, `test`

**Categories**: `feature`, `fix`, `perf`, `security`, `breaking`, `deprecation`, `chore`

### Files & Structure

```bash
agentjj files                               # List all files
agentjj files --pattern "src/**/*.rs"       # Filter by pattern
agentjj files --pattern "*.py" --symbols    # Include symbol counts
```

### Diffs

```bash
agentjj diff                                # Show current diff
agentjj diff --explain                      # With semantic summary
agentjj diff --against @--                  # Compare to 2 changes ago
```

### Push & Apply

```bash
agentjj push                               # Push to remote
agentjj push --pr --title "Fix bug"        # Create PR

agentjj apply \
  --intent "Fix null check" \
  --type behavioral \
  --category fix \
  --patch fix.patch
```

### Self-Documentation

```bash
agentjj schema                             # List all output schemas
agentjj schema --type orient               # Show specific schema
```

## JSON Mode

**Always use `--json` for programmatic access:**

```bash
agentjj --json status
agentjj --json orient
agentjj --json bulk read file1.rs file2.rs
```

Errors also return JSON:
```json
{"error": true, "message": "Symbol not found: foo"}
```

Exit codes: `0` = success, `1` = error

## Agent Manifest

`agentjj init` creates `.agent/manifest.toml`:

```toml
[repo]
name = "my-project"
description = "What this repo does"
languages = ["rust", "python"]
vcs = "jj"

[permissions]
allow = ["src/**", "tests/**"]
deny = ["secrets/**", ".env"]

[[invariants]]
name = "tests-pass"
command = "cargo test"
trigger = "pre-push"
```

The manifest defines:
- **Permissions**: What files agents can modify
- **Invariants**: Commands that must pass (tests, lints, etc.)

## Git Compatibility

agentjj auto-colocates with git repos:

```
my-repo/
├── .git/          # Git still works
├── .jj/           # jj state (auto-created)
├── .agent/        # agentjj config
│   ├── manifest.toml
│   ├── .gitignore     # Excludes local state
│   ├── checkpoints/   # Local (gitignored)
│   └── changes/       # Local (gitignored)
└── src/
```

- `git push`, `git pull`, `git log` all work
- GitHub PRs, issues, actions—all work
- Full git history visible in `agentjj graph`

## Supported Languages

Symbol extraction works for:
- Python
- Rust
- JavaScript
- TypeScript

## Philosophy

1. **Everything is JSON** — `--json` for machine-parseable output
2. **Self-documenting** — `agentjj schema` shows all output formats
3. **Safe by default** — Checkpoints and undo for easy recovery
4. **Batch-friendly** — Bulk operations for efficiency
5. **Context-rich** — Get exactly what you need without bloat

## Development

```bash
# Build
cargo build

# Test
cargo test

# Run locally
./target/debug/agentjj orient
```

## License

MIT
