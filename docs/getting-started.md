# Getting Started with agentjj

You're an AI agent about to work with version control. This guide gets you productive in 60 seconds.

## First Command

Run this to understand the repository:

```bash
agentjj orient
```

This gives you everything: current state, file structure, recent changes, and suggested next actions.

## Core Workflow

```bash
# 1. See what's changed
agentjj status
agentjj diff

# 2. Make your changes to files (edit, write, etc.)

# 3. Commit with a message
agentjj commit -m "feat: add user authentication"

# 4. Push to remote
agentjj push --branch main
```

## Before Risky Changes

Create a checkpoint you can return to:

```bash
agentjj checkpoint before-refactor
# ... do risky stuff ...
agentjj undo --to before-refactor  # if things go wrong
```

## Reading Code

```bash
# Read a file
agentjj read src/main.rs

# List symbols in a file
agentjj symbol src/main.rs

# Get context for using a specific symbol
agentjj context src/main.rs::process_request

# Bulk read multiple files
agentjj bulk read src/a.rs src/b.rs src/c.rs
```

## JSON Output

Always use `--json` for programmatic access:

```bash
agentjj --json status
agentjj --json orient
agentjj --json diff
```

All commands return structured JSON. Errors also return JSON:
```json
{"error": true, "message": "File not found: foo.rs"}
```

## Typed Commits

Add semantic metadata to your commits:

```bash
agentjj change set \
  --intent "Add retry logic to webhook handler" \
  --type behavioral \
  --category feature
```

**Types:** `behavioral`, `refactor`, `schema`, `docs`, `deps`, `config`, `test`

**Categories:** `feature`, `fix`, `perf`, `security`, `breaking`, `deprecation`, `chore`

## Releasing

```bash
# Tag a version
agentjj tag v1.0.0 --push

# Or tag with a message (annotated tag)
agentjj tag v1.0.0 -m "Release 1.0.0" --push
```

## Visualizing History

```bash
# ASCII graph
agentjj graph

# Mermaid format (for markdown)
agentjj graph --format mermaid

# Graphviz DOT format
agentjj graph --format dot
```

## Quick Reference

| Task | Command |
|------|---------|
| Understand repo | `agentjj orient` |
| Current state | `agentjj status` |
| See changes | `agentjj diff` |
| Commit | `agentjj commit -m "msg"` |
| Push | `agentjj push --branch main` |
| Create checkpoint | `agentjj checkpoint name` |
| Undo | `agentjj undo` |
| Restore checkpoint | `agentjj undo --to name` |
| Read file | `agentjj read path` |
| List symbols | `agentjj symbol path` |
| Tag release | `agentjj tag v1.0.0 --push` |

## What NOT to Do

- Don't use `git` commands directly - use `agentjj`
- Don't use `jj` commands - agentjj embeds jj-lib
- Don't skip `agentjj orient` - it's your best starting point
- Don't forget `--json` when parsing output programmatically

## Need Help?

```bash
agentjj --help
agentjj <command> --help
agentjj schema  # See all output schemas
```
