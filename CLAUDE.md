# ABOUTME: Instructions for AI agents working on the agentjj codebase
# ABOUTME: Enforces dogfooding - use agentjj for all version control operations

# agentjj Development Guide

## Critical Rule: Dogfooding Required

**You MUST use `agentjj` for ALL version control operations.** Do not use:
- `git commit`, `git push`, `git tag` → use `agentjj commit`, `agentjj push`, `agentjj tag`
- `jj` commands → agentjj embeds jj-lib, no jj CLI needed
- `gh` for git operations → use agentjj

If agentjj can't do something you need, **add the feature to agentjj first**, then use it.

## Quick Reference

```bash
# Orientation (start here)
agentjj orient

# Working with code
agentjj status              # Current state
agentjj diff                # See changes
agentjj commit -m "msg"     # Commit changes
agentjj push --branch main  # Push to remote
agentjj tag v0.x.x --push   # Tag and push

# Code intelligence
agentjj symbol src/main.rs  # List symbols
agentjj read src/lib.rs     # Read file content

# Safety
agentjj checkpoint before-dangerous-thing
agentjj undo --to before-dangerous-thing
```

## Project Structure

```
src/
├── main.rs      # CLI entry point, all commands
├── lib.rs       # Library exports
├── repo.rs      # Repository operations (jj-lib integration)
├── manifest.rs  # .agent/manifest.toml handling
├── change.rs    # Typed change metadata
├── intent.rs    # Intent/transaction system
└── symbols.rs   # Tree-sitter symbol extraction
tests/
├── cli.rs       # CLI integration tests
└── scenarios.rs # End-to-end scenario tests
```

## Development Workflow

### Building
```bash
cargo build                    # Debug build
cargo build --release          # Release build
cargo test                     # Run all tests
```

### Testing Changes
```bash
# Use the local build for testing
./target/release/agentjj status
./target/release/agentjj commit -m "your message"
./target/release/agentjj push --branch main
```

### Releasing
```bash
# 1. Ensure all tests pass
cargo test

# 2. Update version in Cargo.toml if needed

# 3. Commit and push
./target/release/agentjj commit -m "release: prepare vX.Y.Z"
./target/release/agentjj push --branch main

# 4. Tag and push (triggers GitHub Actions release)
./target/release/agentjj tag vX.Y.Z --push
```

## Architecture Notes

### Self-Contained Design
agentjj embeds `jj-lib` for repository operations but uses `git` directly for network operations (push/fetch). This is because:
1. jj-lib itself uses git subprocess for network operations
2. git is universally available; jj is not
3. Colocated mode means git and jj share the same underlying repo

### Key Design Decisions
- **JSON-first output**: All commands support `--json` for machine parsing
- **Git colocated**: Auto-initializes jj alongside existing git repos
- **Checkpoints**: Named restore points for safe experimentation
- **Typed changes**: Commits have semantic type (behavioral, refactor, etc.)

## Pre-commit Hooks

This repo uses `prek` for pre-commit hooks. The hooks run:
- `cargo fmt --check` - Code formatting
- `cargo clippy` - Linting
- `cargo test` - All tests must pass

Never bypass hooks with `--no-verify`.

## CI/CD

GitHub Actions workflows:
- **ci.yml**: Runs on every push/PR - format, clippy, test
- **release.yml**: Triggered by version tags - builds binaries, creates GitHub release, updates Homebrew tap

## Adding New Commands

1. Add variant to `Commands` enum in `src/main.rs`
2. Add match arm in `main()` function
3. Implement `cmd_<name>` function
4. Support `--json` output mode
5. Add tests in `tests/cli.rs`
6. Update README.md

## Testing Philosophy

- Unit tests for helper functions
- Integration tests for CLI commands
- Scenario tests for end-to-end workflows
- All tests must pass before commit (enforced by pre-commit)
