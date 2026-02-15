# AI Agents Outgrew Git

The first generation of AI coding agents needed three git commands: `status`, `commit`, `push`. Git handled that fine. You shell out, parse some text, move on. It worked.

Then agents got better.

They started reading entire codebases, reasoning about architecture, refactoring across modules, experimenting with approaches and rolling back the bad ones. They went from "apply this patch" to "understand this system and improve it." And somewhere in that leap, git stopped being enough — not because it broke, but because agents started needing things git was never designed to give them.

## What Changed

**Early agents** lived in a tight loop: receive instruction, edit file, commit, push. Git's human CLI was fine for this because the agent was basically a typist with better pattern matching.

**Current agents** operate more like junior developers. They need to understand repo state before acting. They read dozens of files to build context. They try an approach, realize it's wrong, back up, and try another. They need to know what a function does without reading the entire file it lives in. They work in batches because every round-trip costs tokens and time.

Git doesn't have answers for any of this. Not because git is bad — git is excellent at what it does — but because these aren't version control questions. They're *agent workflow* questions, and they just happen to intersect with version control.

So agents compensate. They wrap git in parsing layers. They make sequential calls when they need batch results. They read whole files when they need one function signature. They have no rollback beyond `git stash` and hope. It all works. It's all friction.

## The Friction You Stop Noticing

Here's the thing about friction that works: you stop seeing it. The agent makes 12 shell calls to understand a repo that could be one call. The agent reads 400 lines to get a 3-line function signature. The agent has no safe way to experiment, so it commits to the first approach and patches mistakes forward.

None of this causes a crash. The agent completes the task. But it's slower, costs more tokens, and produces worse results than it would with the right interface. The gap between "agent with git" and "agent with purpose-built VCS tooling" isn't dramatic on any single task. It's cumulative. It's death by a thousand round-trips.

## What We Built

**agentjj** is version control tooling designed for where agents are now, not where they were two years ago. It's a single Rust binary that embeds [Jujutsu (jj)](https://github.com/martinvonz/jj)'s library, auto-colocates with existing git repos, and speaks JSON natively.

Git keeps working. Your CI keeps working. Your human workflow doesn't change. The agent just gets a better interface.

```bash
brew install 2389-research/tap/agentjj
cd your-git-repo
agentjj orient
```

### Situational Awareness in One Call

The first thing any agent should do in a repo is understand what's going on. `orient` gives a complete briefing — branch state, recent changes, modified files, repo stats, suggested next action — in one round-trip.

```bash
agentjj orient --json
```

Compare that to the typical agent preamble: `git status`, `git log --oneline -10`, `git branch -a`, `git diff --stat`, then parse all four outputs. Four calls, four parsing steps, still less information than `orient` returns.

### Code Intelligence at the VCS Layer

This is where the gap between "git works fine" and "git isn't enough" gets concrete. Agents constantly need to understand code structure — what functions exist, what they do, what depends on them. Git doesn't know or care. It sees files and lines.

agentjj uses tree-sitter to parse actual ASTs:

```bash
agentjj symbol src/main.rs                # all symbols in a file
agentjj symbol src/main.rs::handle_push   # one specific symbol
agentjj context src/main.rs::handle_push  # signature + docstring, nothing else
agentjj affected src/main.rs::handle_push # impact analysis
```

Works across Python, Rust, JavaScript, and TypeScript. The agent gets exactly the context it needs without stuffing entire files into its context window. This isn't a nice-to-have — for agents operating under token budgets, it's the difference between understanding 5 functions and understanding 50.

### Batch Operations

Agents have limited turns. Every shell call is a turn. agentjj lets them batch:

```bash
agentjj bulk read src/main.rs src/repo.rs src/lib.rs
agentjj bulk symbols "src/**/*.rs"
agentjj bulk context src/repo.rs::commit src/repo.rs::push
```

One call, multiple results. This sounds like a minor optimization until you realize a typical refactoring task involves reading 15-20 files. That's 15-20 turns with git. It's 1 turn with agentjj.

### Safe Experimentation

Good agents explore. They try an approach, evaluate it, and sometimes abandon it. Git makes this surprisingly hard to do programmatically. `git stash` is a stack with no names. `git reflog` is powerful but hostile to machine parsing. Interactive rebase is... interactive.

agentjj has named checkpoints:

```bash
agentjj checkpoint before-refactor
# agent experiments freely
agentjj undo --to before-refactor  # clean, named rollback
```

Stored locally, invisible to git, one-command recovery. The agent can be aggressive because backing up is trivial.

### Typed Commits

Every commit carries structured metadata beyond the message:

```bash
agentjj commit -m "feat: add bulk symbol query"
```

agentjj tracks change type (behavioral, refactor, schema, docs), category (feature, fix, perf, security), and intent as machine-readable data. Downstream tools can generate changelogs, assess risk, or route reviews — without parsing commit message conventions via regex.

## The Governance Layer

As agents get more capable, the question shifts from "can the agent do this?" to "should the agent do this?" agentjj includes a manifest system for exactly this:

```toml
# .agent/manifest.toml
[permissions]
allow_change = ["src/**", "tests/**"]
deny_change = [".agent/*", "migrations/*"]

[invariants]
tests_pass = { cmd = "cargo test", on = ["pre-push"] }

[review]
require_human = ["src/billing/*", "migrations/*"]
```

The agent knows its boundaries before it starts working. Certain paths require human review. Certain invariants must pass. This isn't security theater — it's the difference between "agent with full repo access" and "agent with scoped, auditable repo access."

## Why Embed jj-lib?

We made one architectural bet: embed Jujutsu's library for repo operations, use git for networking.

jj gives us a superior change model — first-class conflicts, an operation log, content-addressed storage. But jj-lib itself uses git for push and fetch. So rather than requiring two CLI tools, agentjj ships as one binary with jj-lib embedded and git handling network operations.

The result: agents get jj's powerful internals through a purpose-built interface, teams keep their existing git infrastructure, and nobody installs anything beyond agentjj.

## The Real Question

The question isn't "does git work for agents?" It does. The question is: now that agents are doing real engineering work — reading codebases, reasoning about impact, experimenting with approaches, working under token and turn constraints — is "works" the bar we're aiming for?

We think agents deserve tooling that's designed for how they actually operate. Not wrappers on human interfaces. Not parsing layers on text output. Actual, purpose-built tools.

That's what agentjj is.

## Get Started

```bash
# Install
brew install 2389-research/tap/agentjj
# or
cargo install agentjj

# Try it
cd your-repo
agentjj orient
agentjj status --json
```

Open source. Single binary. macOS and Linux.

**GitHub:** [github.com/2389-research/agentjj](https://github.com/2389-research/agentjj)

---

*Built by [2389 Research](https://github.com/2389-research).*
