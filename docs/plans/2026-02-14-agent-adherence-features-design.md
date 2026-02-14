# ABOUTME: Design doc for three agent-adherence features: selective commit, checkpoint list, richer graph
# ABOUTME: Based on field observations from 7 projects, 100+ interactions, 17K+ turns

# Agent Adherence Features Design

Three features to improve agent adherence to agentjj, informed by real session data.

## Feature 1: `--paths` flag on commit

### Problem
Agents commit the entire working copy. When multiple subagents work in parallel,
Agent A's commit sweeps up Agent B's half-finished work. The first commit in a
repo also picks up stray files.

### Design
Add `--paths <file>...` to `agentjj commit`. When specified, only changes to
the listed paths are included in the commit. Unlisted changes remain in the
working copy.

**Implementation approach: Tree-level filtering**

After snapshot, build a selective tree:
1. Start with parent_tree (the baseline)
2. For each path in `--paths`, copy that path's value from new_tree (snapshot)
3. The result is a tree with ONLY the specified files changed

This works within jj-lib's tree model. The `TreeBuilder` API lets us construct
trees by setting individual path values.

**CLI:**
```
agentjj commit -m "feat: add lexer" --paths src/lexer.rs src/lexer_test.rs
```

**Behavior when --paths is omitted:** Unchanged (commit everything).

### Edge cases
- Path doesn't exist in snapshot: error with clear message
- Path is unchanged: skip silently (no-op for that path)
- Empty result after filtering: error "no changes in specified paths"

## Feature 2: `checkpoint list`

### Problem
Agents can't see what checkpoints exist. They guess or hack with `ls`.

### Design
Add `list` as a subcommand/mode to `checkpoint`:
```
agentjj checkpoint list [--json]
```

Scans `.agent/checkpoints/*.json`, parses each, sorts by `created_at` desc.

**Output (human):**
```
Checkpoints:
  phase-8b-ios-app        2026-02-14 10:23:15  "iOS app checkpoint"
  phase-8a-hermit-ffi     2026-02-14 10:15:02  "FFI layer done"
  before-refactor         2026-02-14 09:45:30  (no description)
```

**Output (JSON):**
```json
{
  "checkpoints": [
    {
      "name": "phase-8b-ios-app",
      "description": "iOS app checkpoint",
      "change_id": "abc123",
      "operation_id": "def456",
      "created_at": "2026-02-14T10:23:15Z"
    }
  ]
}
```

**When no checkpoints exist:** "No checkpoints found."

## Feature 3: Richer graph output

### Problem
Agents fall back to `git log` for timestamps, authors, and full commit hashes.

### Design
Extend `LogEntry` in repo.rs with:
- `timestamp: Option<String>` (ISO 8601 from commit metadata)
- `author: Option<String>` (from commit metadata)
- `full_commit_id: String` (unhashed, for git cross-reference)

Update graph output formats to include these fields. ASCII format gets timestamps
inline. JSON always includes all fields.

---

## Implementation Order

1. `checkpoint list` (smallest, quickest win)
2. Richer graph (extends existing structs)
3. `--paths` on commit (most complex, touches commit core)
