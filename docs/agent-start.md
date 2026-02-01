---
name: agentjj
version: 0.1.1
description: Agent-first version control - a self-contained porcelain for jj/git
homepage: https://github.com/2389-research/agentjj
tags: [version-control, git, jj, cli, agent-tools]
---

# agentjj

Agent-first version control. Self-contained binary that works with any git repo.

## Installation

### Homebrew (macOS/Linux)

```bash
brew install 2389-research/tap/agentjj
```

### Cargo (Rust)

```bash
cargo install agentjj
```

### Binary Download

```bash
# macOS ARM (M1/M2/M3)
curl -L https://github.com/2389-research/agentjj/releases/latest/download/agentjj-aarch64-apple-darwin.tar.gz | tar xz
sudo mv agentjj /usr/local/bin/

# macOS Intel
curl -L https://github.com/2389-research/agentjj/releases/latest/download/agentjj-x86_64-apple-darwin.tar.gz | tar xz
sudo mv agentjj /usr/local/bin/

# Linux x86_64
curl -L https://github.com/2389-research/agentjj/releases/latest/download/agentjj-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv agentjj /usr/local/bin/
```

### Verify Installation

```bash
agentjj --version
```

### Install the Skill (Claude Code)

To give Claude Code agents full knowledge of agentjj commands:

```bash
# Create skills directory
mkdir -p ~/.claude/skills/agentjj

# Download the skill
curl -L https://raw.githubusercontent.com/2389-research/agentjj/main/docs/skill.md \
  -o ~/.claude/skills/agentjj/SKILL.md
```

Or reference it directly in your project's `CLAUDE.md`:

```markdown
## Version Control

Use agentjj for all version control operations.
Skill: @https://raw.githubusercontent.com/2389-research/agentjj/main/docs/skill.md
```

## Quick Start

### 1. Orient Yourself

In any git repository, run:

```bash
agentjj orient
```

This returns everything you need: current state, file structure, recent changes, and suggested actions.

### 2. Core Workflow

```bash
# See current state
agentjj status

# See what changed
agentjj diff

# Make your edits to files...

# Commit changes
agentjj commit -m "feat: add user authentication"

# Push to remote
agentjj push --branch main
```

### 3. Safety First

Before risky operations, create a checkpoint:

```bash
agentjj checkpoint before-refactor

# ... do dangerous stuff ...

# If things go wrong:
agentjj undo --to before-refactor
```

## Commands

### Repository State

| Command | Description |
|---------|-------------|
| `agentjj orient` | Complete repo orientation (start here) |
| `agentjj status` | Current change ID, operation ID, files |
| `agentjj diff` | Show current changes |
| `agentjj graph` | Visualize commit DAG |

### Making Changes

| Command | Description |
|---------|-------------|
| `agentjj commit -m "msg"` | Commit current changes |
| `agentjj push --branch main` | Push to remote |
| `agentjj tag v1.0.0 --push` | Create and push a tag |

### Code Intelligence

| Command | Description |
|---------|-------------|
| `agentjj read <file>` | Read file content |
| `agentjj symbol <file>` | List symbols in file |
| `agentjj context <file>::<symbol>` | Get context for a symbol |
| `agentjj bulk read <files...>` | Read multiple files |

### Safety & Recovery

| Command | Description |
|---------|-------------|
| `agentjj checkpoint <name>` | Create named restore point |
| `agentjj undo` | Undo last operation |
| `agentjj undo --to <name>` | Restore to checkpoint |
| `agentjj undo --dry-run` | Preview what would be undone |

## JSON Mode

**Always use `--json` for programmatic access:**

```bash
agentjj --json status
agentjj --json orient
agentjj --json diff
```

Output is structured JSON. Errors return:
```json
{"error": true, "message": "Description of what went wrong"}
```

Exit codes: `0` = success, `1` = error

## Typed Commits

Add semantic metadata to commits:

```bash
agentjj change set \
  --intent "Add retry logic" \
  --type behavioral \
  --category feature
```

**Types:** `behavioral`, `refactor`, `schema`, `docs`, `deps`, `config`, `test`

**Categories:** `feature`, `fix`, `perf`, `security`, `breaking`, `deprecation`, `chore`

## Graph Formats

```bash
# ASCII (default)
agentjj graph

# Mermaid (for markdown/docs)
agentjj graph --format mermaid

# Graphviz DOT
agentjj graph --format dot

# More commits
agentjj graph --limit 20

# All branches
agentjj graph --all
```

## Important Rules

1. **Use agentjj, not git** - Don't shell out to `git` commands
2. **Use agentjj, not jj** - agentjj embeds jj-lib, no jj CLI needed
3. **Always start with `orient`** - It's your best context source
4. **Use `--json` for parsing** - Human output may change
5. **Checkpoint before risk** - Easy recovery beats careful planning

## Self-Documenting

```bash
# List all output schemas
agentjj schema

# Get specific schema
agentjj schema --type status
agentjj schema --type orient
```

## Help

```bash
agentjj --help
agentjj <command> --help
```

## Links

- Repository: https://github.com/2389-research/agentjj
- Releases: https://github.com/2389-research/agentjj/releases
- Homebrew: `brew install 2389-research/tap/agentjj`
