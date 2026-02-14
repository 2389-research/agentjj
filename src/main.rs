// ABOUTME: CLI entry point for agentjj - agent-oriented jj porcelain
// ABOUTME: Provides commands for manifest, typed changes, intent transactions, and reads

use anyhow::Result;
use clap::{Parser, Subcommand};

use agentjj::change::{ChangeCategory, ChangeType, TypedChange};
use agentjj::intent::{ChangeSpec, Intent, Preconditions};
use agentjj::manifest::Manifest;
use agentjj::repo::Repo;

#[derive(Parser)]
#[command(name = "agentjj")]
#[command(about = "Agent-oriented porcelain for Jujutsu version control")]
#[command(version)]
struct Cli {
    /// Output as JSON (for machine parsing)
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize agentjj in a repository
    Init {
        /// Repository name
        #[arg(short, long)]
        name: Option<String>,
    },

    /// Show repository status (change ID, operation ID, files)
    Status,

    /// Show or validate the manifest
    Manifest {
        #[command(subcommand)]
        action: ManifestAction,
    },

    /// Work with typed changes
    Change {
        #[command(subcommand)]
        action: ChangeAction,
    },

    /// Apply an intent (atomic transaction)
    Apply {
        /// Intent description
        #[arg(short, long)]
        intent: String,

        /// Change type (behavioral, refactor, schema, docs, deps, config, test)
        #[arg(short = 't', long, default_value = "behavioral")]
        r#type: String,

        /// Category (feature, fix, perf, security, breaking, deprecation, chore)
        #[arg(short, long)]
        category: Option<String>,

        /// Patch file to apply
        #[arg(short, long)]
        patch: Option<String>,

        /// Precondition: branch@change_id
        #[arg(long)]
        precondition: Vec<String>,

        /// Skip running invariants
        #[arg(long)]
        no_invariants: bool,

        /// Mark as breaking change
        #[arg(long)]
        breaking: bool,
    },

    /// Read file content at a specific change
    Read {
        /// File path
        path: String,

        /// Change ID or branch (default: @)
        #[arg(short, long)]
        at: Option<String>,
    },

    /// Query symbols in the codebase
    Symbol {
        /// Symbol path (e.g., src/api.py::process_request)
        path: String,

        /// Show only signature
        #[arg(long)]
        signature: bool,
    },

    /// Get minimal context needed to use a symbol
    Context {
        /// Symbol path (e.g., src/api.py::process_request)
        path: String,
    },

    /// Push changes and optionally create a PR
    Push {
        /// Branch name to push to
        #[arg(short, long)]
        branch: Option<String>,

        /// Change ID to push (default: @- if @ is empty, else @)
        #[arg(short, long)]
        change: Option<String>,

        /// Create a pull request
        #[arg(long)]
        pr: bool,

        /// PR title (required if --pr)
        #[arg(long)]
        title: Option<String>,

        /// PR body
        #[arg(long)]
        body: Option<String>,

        /// Target branch for PR (default: main)
        #[arg(long, default_value = "main")]
        target: String,
    },

    /// Commit current changes with a message (describe + new)
    Commit {
        /// Commit message
        #[arg(short, long)]
        message: String,

        /// Don't create a new working copy after committing
        #[arg(long)]
        no_new: bool,

        /// Change type (behavioral, refactor, schema, docs, deps, config, test)
        #[arg(short = 't', long = "type", default_value = "behavioral")]
        change_type: String,

        /// Category (feature, fix, perf, security, breaking, deprecation, chore)
        #[arg(short, long)]
        category: Option<String>,

        /// Skip running invariants
        #[arg(long)]
        no_invariants: bool,

        /// Mark as breaking change
        #[arg(long)]
        breaking: bool,

        /// Only include changes to these paths in the commit
        #[arg(long, num_args = 1..)]
        paths: Option<Vec<String>>,
    },

    /// Create or update a git tag
    Tag {
        /// Tag name (e.g., v0.1.0)
        name: String,

        /// Tag message (creates annotated tag)
        #[arg(short, long)]
        message: Option<String>,

        /// Force update if tag exists
        #[arg(short, long)]
        force: bool,

        /// Push tag to remote
        #[arg(long)]
        push: bool,
    },

    /// Complete repository orientation for agents - everything you need to start working
    Orient,

    /// Checkpoint operations (create, list)
    Checkpoint {
        #[command(subcommand)]
        action: CheckpointAction,
    },

    /// Undo the last operation (restore to previous state)
    Undo {
        /// Number of operations to undo (default: 1)
        #[arg(short, long, default_value = "1")]
        steps: usize,

        /// Restore to a named checkpoint
        #[arg(long, conflicts_with = "steps")]
        to: Option<String>,

        /// Dry run - show what would be undone without doing it
        #[arg(long)]
        dry_run: bool,
    },

    /// Bulk operations for efficiency
    Bulk {
        #[command(subcommand)]
        action: BulkAction,
    },

    /// Show what files exist and their properties
    Files {
        /// Glob pattern to filter files
        #[arg(short, long)]
        pattern: Option<String>,

        /// Include symbol counts per file
        #[arg(long)]
        symbols: bool,
    },

    /// Show semantic diff of current changes
    Diff {
        /// Compare against this revision (default: @-)
        #[arg(short, long)]
        against: Option<String>,

        /// Include AI-generated explanation of changes
        #[arg(long)]
        explain: bool,
    },

    /// Analyze what would be affected by changing a symbol
    Affected {
        /// Symbol to analyze (e.g., src/api.rs::process)
        symbol: String,

        /// Depth of dependency analysis (default: 2)
        #[arg(short, long, default_value = "2")]
        depth: usize,
    },

    /// Print JSON schemas for all output types (self-documenting)
    Schema {
        /// Specific type to show schema for
        #[arg(short, long)]
        r#type: Option<String>,
    },

    /// Validate current changes are complete and ready
    Validate,

    /// Suggest next actions based on current state
    Suggest,

    /// Output the full skill documentation (for agent self-discovery)
    Skill,

    /// Show a concise getting-started guide (works without a repo)
    Quickstart,

    /// Output the repository DAG in various formats
    Graph {
        /// Output format: ascii (default), mermaid, dot (graphviz)
        #[arg(long, default_value = "ascii")]
        format: String,

        /// Number of commits to show (default: 10)
        #[arg(long, default_value = "10")]
        limit: usize,

        /// Show all branches, not just current
        #[arg(long)]
        all: bool,
    },
}

#[derive(Subcommand)]
enum BulkAction {
    /// Read multiple files at once
    Read {
        /// File paths (space-separated)
        paths: Vec<String>,
    },

    /// Query symbols across multiple files
    Symbols {
        /// Glob pattern (e.g., "src/**/*.rs")
        pattern: String,

        /// Only show public symbols
        #[arg(long)]
        public_only: bool,
    },

    /// Get context for multiple symbols
    Context {
        /// Symbol paths (e.g., "src/a.rs::foo src/b.rs::bar")
        symbols: Vec<String>,
    },
}

#[derive(Subcommand)]
enum ManifestAction {
    /// Show the current manifest
    Show,

    /// Validate the manifest
    Validate,

    /// Initialize a new manifest
    Init {
        /// Repository name
        #[arg(short, long)]
        name: String,
    },
}

#[derive(Subcommand)]
enum ChangeAction {
    /// Show typed change metadata
    Show {
        /// Change ID
        change_id: String,
    },

    /// List all typed changes
    List {
        /// Filter by type
        #[arg(short = 't', long)]
        r#type: Option<String>,

        /// Show only breaking changes
        #[arg(long)]
        breaking: bool,
    },

    /// Add or update typed change metadata
    Set {
        /// Change ID (default: current)
        #[arg(long)]
        change_id: Option<String>,

        /// Intent description
        #[arg(short, long)]
        intent: String,

        /// Change type
        #[arg(short = 't', long)]
        r#type: String,

        /// Category
        #[arg(short = 'c', long)]
        category: Option<String>,

        /// Mark as breaking
        #[arg(long)]
        breaking: bool,
    },
}

#[derive(Subcommand)]
enum CheckpointAction {
    /// Create a named checkpoint for easy recovery
    Create {
        /// Checkpoint name
        name: String,

        /// Description of what state this captures
        #[arg(short, long)]
        description: Option<String>,
    },

    /// List all checkpoints
    List,
}

fn main() {
    let cli = Cli::parse();
    let json_mode = cli.json;

    let result = run_command(cli);

    if let Err(e) = result {
        if json_mode {
            println!(
                "{}",
                serde_json::json!({
                    "error": true,
                    "message": e.to_string()
                })
            );
        } else {
            eprintln!("Error: {}", e);
        }
        std::process::exit(1);
    }
}

fn run_command(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Init { name } => cmd_init(name, cli.json),
        Commands::Status => cmd_status(cli.json),
        Commands::Manifest { action } => cmd_manifest(action, cli.json),
        Commands::Change { action } => cmd_change(action, cli.json),
        Commands::Apply {
            intent,
            r#type,
            category,
            patch,
            precondition,
            no_invariants,
            breaking,
        } => cmd_apply(
            intent,
            r#type,
            category,
            patch,
            precondition,
            no_invariants,
            breaking,
            cli.json,
        ),
        Commands::Read { path, at } => cmd_read(path, at, cli.json),
        Commands::Symbol { path, signature } => cmd_symbol(path, signature, cli.json),
        Commands::Context { path } => cmd_context(path, cli.json),
        Commands::Push {
            branch,
            change,
            pr,
            title,
            body,
            target,
        } => cmd_push(branch, change, pr, title, body, target, cli.json),
        Commands::Commit {
            message,
            no_new,
            change_type,
            category,
            no_invariants,
            breaking,
            paths,
        } => cmd_commit(
            message,
            no_new,
            change_type,
            category,
            no_invariants,
            breaking,
            paths,
            cli.json,
        ),
        Commands::Tag {
            name,
            message,
            force,
            push,
        } => cmd_tag(name, message, force, push, cli.json),
        Commands::Orient => cmd_orient(cli.json),
        Commands::Checkpoint { action } => match action {
            CheckpointAction::Create { name, description } => {
                cmd_checkpoint(name, description, cli.json)
            }
            CheckpointAction::List => cmd_checkpoint_list(cli.json),
        },
        Commands::Undo { steps, to, dry_run } => cmd_undo(steps, to, dry_run, cli.json),
        Commands::Bulk { action } => cmd_bulk(action, cli.json),
        Commands::Files { pattern, symbols } => cmd_files(pattern, symbols, cli.json),
        Commands::Diff { against, explain } => cmd_diff(against, explain, cli.json),
        Commands::Affected { symbol, depth } => cmd_affected(symbol, depth, cli.json),
        Commands::Schema { r#type } => cmd_schema(r#type, cli.json),
        Commands::Validate => cmd_validate(cli.json),
        Commands::Suggest => cmd_suggest(cli.json),
        Commands::Skill => cmd_skill(cli.json),
        Commands::Quickstart => cmd_quickstart(cli.json),
        Commands::Graph { format, limit, all } => cmd_graph(format, limit, all, cli.json),
    }
}

fn cmd_init(name: Option<String>, json: bool) -> Result<()> {
    let repo = Repo::discover()?;

    if repo.has_manifest() {
        if json {
            println!(r#"{{"status": "exists", "path": ".agent/manifest.toml"}}"#);
        } else {
            println!("Manifest already exists at .agent/manifest.toml");
        }
        return Ok(());
    }

    let repo_name = name.unwrap_or_else(|| {
        repo.root()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed")
            .to_string()
    });

    let manifest = Manifest {
        repo: agentjj::manifest::RepoInfo {
            name: repo_name.clone(),
            description: String::new(),
            languages: Vec::new(),
            vcs: "jj".to_string(),
        },
        ..Default::default()
    };

    let manifest_path = repo.root().join(Manifest::DEFAULT_PATH);
    if let Some(parent) = manifest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&manifest_path, manifest.to_toml()?)?;

    // Create .agent/.gitignore to exclude local state but track manifest
    let agent_gitignore = repo.root().join(".agent/.gitignore");
    let gitignore_content = "# Agent-local state (not shared)\n\
                             checkpoints/\n\
                             changes/\n";
    std::fs::write(&agent_gitignore, gitignore_content)?;

    if json {
        println!(
            r#"{{"status": "created", "name": "{}", "path": ".agent/manifest.toml", "gitignore": ".agent/.gitignore"}}"#,
            repo_name
        );
    } else {
        println!("Initialized agentjj for '{}'", repo_name);
        println!("Created .agent/manifest.toml");
        println!("Created .agent/.gitignore (excludes local state)");
    }

    Ok(())
}

fn cmd_status(json: bool) -> Result<()> {
    let mut repo = Repo::discover()?;

    let change_id = repo
        .current_change_id()
        .unwrap_or_else(|_| "unknown".into());
    let operation_id = repo
        .current_operation_id()
        .unwrap_or_else(|_| "unknown".into());
    let files = repo.changed_files(&change_id).unwrap_or_default();
    let has_manifest = repo.has_manifest();

    // Try to load typed change for current change
    let typed_change = repo.get_typed_change(&change_id).ok();

    if json {
        let status = serde_json::json!({
            "change_id": change_id,
            "operation_id": operation_id,
            "files_changed": files,
            "has_manifest": has_manifest,
            "typed_change": typed_change,
        });
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("Change:    {}", &change_id[..12.min(change_id.len())]);
        println!(
            "Operation: {}...",
            &operation_id[..16.min(operation_id.len())]
        );
        println!("Manifest:  {}", if has_manifest { "yes" } else { "no" });

        if !files.is_empty() {
            println!("\nChanged files:");
            for f in &files {
                println!("  {}", f);
            }
        }

        if let Some(tc) = typed_change {
            println!("\nTyped change:");
            println!("  type:   {:?}", tc.change_type);
            println!("  intent: {}", tc.intent);
            if tc.breaking {
                println!("  ⚠️  BREAKING CHANGE");
            }
        }
    }

    Ok(())
}

fn cmd_manifest(action: ManifestAction, json: bool) -> Result<()> {
    match action {
        ManifestAction::Show => {
            let mut repo = Repo::discover()?;
            let manifest = repo.manifest()?;
            if json {
                println!("{}", serde_json::to_string_pretty(manifest)?);
            } else {
                println!("{}", manifest.to_toml()?);
            }
        }
        ManifestAction::Validate => {
            let mut repo = Repo::discover()?;
            match repo.manifest() {
                Ok(m) => {
                    if json {
                        println!(r#"{{"valid": true, "name": "{}"}}"#, m.repo.name);
                    } else {
                        println!("✓ Manifest is valid");
                        println!("  name: {}", m.repo.name);
                        println!("  invariants: {}", m.invariants.len());
                    }
                }
                Err(e) => {
                    if json {
                        println!(r#"{{"valid": false, "error": "{}"}}"#, e);
                    } else {
                        println!("✗ Manifest is invalid: {}", e);
                    }
                    std::process::exit(1);
                }
            }
        }
        ManifestAction::Init { name } => {
            return cmd_init(Some(name), json);
        }
    }
    Ok(())
}

fn cmd_change(action: ChangeAction, json: bool) -> Result<()> {
    let mut repo = Repo::discover()?;

    match action {
        ChangeAction::Show { change_id } => {
            let change = repo.get_typed_change(&change_id)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&change)?);
            } else {
                println!("{}", change.to_toml()?);
            }
        }
        ChangeAction::List { r#type, breaking } => {
            let index = agentjj::change::ChangeIndex::load_from_repo(repo.root())?;

            let changes: Vec<_> = if breaking {
                index.breaking_changes()
            } else if let Some(type_str) = r#type {
                let change_type = parse_change_type(&type_str)?;
                index.by_type(change_type)
            } else {
                index.all()
            };

            if json {
                println!("{}", serde_json::to_string_pretty(&changes)?);
            } else if changes.is_empty() {
                println!("No typed changes found");
            } else {
                for change in changes {
                    println!(
                        "{} [{:?}] {}",
                        change.change_id, change.change_type, change.intent
                    );
                }
            }
        }
        ChangeAction::Set {
            change_id,
            intent,
            r#type,
            category,
            breaking,
        } => {
            // Resolve @ to actual jj change ID
            let cid = match change_id {
                Some(id) if id != "@" => id,
                _ => repo.current_change_id()?,
            };
            let change_type = parse_change_type(&r#type)?;
            let category = category.map(|c| parse_category(&c)).transpose()?;

            let mut change = TypedChange::new(cid.clone(), change_type, intent);
            if let Some(cat) = category {
                change = change.with_category(cat);
            }
            if breaking {
                change = change.breaking();
            }

            repo.save_typed_change(&change)?;

            if json {
                println!("{}", serde_json::to_string_pretty(&change)?);
            } else {
                println!("Saved typed change for {}", cid);
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_apply(
    intent_desc: String,
    type_str: String,
    category: Option<String>,
    patch: Option<String>,
    preconditions: Vec<String>,
    no_invariants: bool,
    breaking: bool,
    json: bool,
) -> Result<()> {
    let mut repo = Repo::discover()?;

    let change_type = parse_change_type(&type_str)?;

    // Build change spec
    let changes = if let Some(patch_file) = patch {
        let content = std::fs::read_to_string(&patch_file)?;
        ChangeSpec::Patch { content }
    } else {
        anyhow::bail!("--patch is required (for now)");
    };

    // Build preconditions
    let mut preconds = Preconditions::default();
    for p in preconditions {
        if let Some((branch, change_id)) = p.split_once('@') {
            preconds = preconds.with_branch_at(branch, change_id);
        } else {
            anyhow::bail!("Invalid precondition format: {}. Use branch@change_id", p);
        }
    }

    // Build intent
    let mut intent = Intent::new(intent_desc, change_type, changes).with_preconditions(preconds);

    if let Some(cat) = category {
        intent = intent.with_category(parse_category(&cat)?);
    }
    if no_invariants {
        intent = intent.skip_invariants();
    }
    if breaking {
        intent = intent.breaking();
    }

    // Apply
    let result = repo.apply(intent)?;

    let is_success = matches!(&result, agentjj::intent::IntentResult::Success { .. });

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        match &result {
            agentjj::intent::IntentResult::Success { change_id, .. } => {
                println!("✓ Applied successfully");
                println!("  change: {}", change_id);
            }
            agentjj::intent::IntentResult::Conflict { conflicts, .. } => {
                println!("✗ Conflict in {} files", conflicts.len());
            }
            agentjj::intent::IntentResult::PreconditionFailed {
                reason,
                expected,
                actual,
            } => {
                println!("✗ Precondition failed: {}", reason);
                println!("  expected: {}", expected);
                println!("  actual: {}", actual);
            }
            agentjj::intent::IntentResult::InvariantFailed {
                invariant,
                stderr,
                exit_code,
                ..
            } => {
                println!("✗ Invariant '{}' failed (exit {})", invariant, exit_code);
                if !stderr.is_empty() {
                    println!("  stderr: {}", stderr);
                }
            }
            agentjj::intent::IntentResult::PermissionDenied {
                path, action, rule, ..
            } => {
                println!(
                    "✗ Permission denied: {} on '{}' (rule: {})",
                    action, path, rule
                );
            }
            agentjj::intent::IntentResult::RequiresReview { message, paths, .. } => {
                println!("⚠ Requires human review: {}", message);
                if !paths.is_empty() {
                    println!("  paths: {}", paths.join(", "));
                }
            }
        }
    }

    if !is_success {
        std::process::exit(1);
    }

    Ok(())
}

fn cmd_read(path: String, at: Option<String>, json: bool) -> Result<()> {
    let mut repo = Repo::discover()?;
    let content = repo.read_file(&path, at.as_deref())?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "path": path,
                "at": at,
                "content": content
            }))?
        );
    } else {
        print!("{}", content);
    }

    Ok(())
}

fn cmd_symbol(path: String, signature_only: bool, json: bool) -> Result<()> {
    // Parse path: can be "file.py" or "file.py::symbol_name"
    let (file_path, symbol_name) = if let Some(idx) = path.find("::") {
        (&path[..idx], Some(&path[idx + 2..]))
    } else {
        (path.as_str(), None)
    };

    let file_path_obj = std::path::Path::new(file_path);

    // Detect language
    let lang = agentjj::SupportedLanguage::from_path(file_path_obj)
        .ok_or_else(|| anyhow::anyhow!("Unsupported file type: {}", file_path))?;

    // Read file content - use filesystem for absolute paths, jj for relative
    let content = if file_path_obj.is_absolute() {
        std::fs::read_to_string(file_path)?
    } else {
        let mut repo = Repo::discover()?;
        repo.read_file(file_path, None)?
    };

    if let Some(name) = symbol_name {
        // Find specific symbol
        let symbol = agentjj::symbols::find_symbol(&content, lang, name)?;

        match symbol {
            Some(s) => {
                if json {
                    if signature_only {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "name": s.name,
                                "signature": s.signature,
                            }))?
                        );
                    } else {
                        println!("{}", serde_json::to_string_pretty(&s)?);
                    }
                } else if signature_only {
                    if let Some(sig) = &s.signature {
                        println!("{}", sig);
                    } else {
                        println!("{}", s.name);
                    }
                } else {
                    println!("{} ({:?})", s.name, s.kind);
                    if let Some(sig) = &s.signature {
                        println!("  {}", sig);
                    }
                    println!("  lines {}-{}", s.start_line, s.end_line);
                }
            }
            None => {
                if json {
                    println!(r#"{{"error": "symbol not found", "name": "{}"}}"#, name);
                } else {
                    println!("Symbol '{}' not found in {}", name, file_path);
                }
                std::process::exit(1);
            }
        }
    } else {
        // List all symbols in file
        let symbols = agentjj::symbols::extract_symbols(&content, lang)?;

        if json {
            println!("{}", serde_json::to_string_pretty(&symbols)?);
        } else {
            for s in symbols {
                let sig = s.signature.as_deref().unwrap_or(&s.name);
                let truncated = if sig.len() > 60 {
                    format!("{}...", &sig[..57])
                } else {
                    sig.to_string()
                };
                println!(
                    "{:>4} {:10} {}",
                    s.start_line,
                    format!("{:?}", s.kind).to_lowercase(),
                    truncated
                );
            }
        }
    }

    Ok(())
}

fn parse_change_type(s: &str) -> Result<ChangeType> {
    match s.to_lowercase().as_str() {
        "behavioral" | "behavior" => Ok(ChangeType::Behavioral),
        "refactor" => Ok(ChangeType::Refactor),
        "schema" => Ok(ChangeType::Schema),
        "docs" | "doc" => Ok(ChangeType::Docs),
        "deps" | "dependency" | "dependencies" => Ok(ChangeType::Deps),
        "config" | "configuration" => Ok(ChangeType::Config),
        "test" | "tests" => Ok(ChangeType::Test),
        _ => anyhow::bail!("Unknown change type: {}", s),
    }
}

fn parse_category(s: &str) -> Result<ChangeCategory> {
    match s.to_lowercase().as_str() {
        "feature" | "feat" => Ok(ChangeCategory::Feature),
        "fix" | "bugfix" => Ok(ChangeCategory::Fix),
        "perf" | "performance" => Ok(ChangeCategory::Perf),
        "security" | "sec" => Ok(ChangeCategory::Security),
        "breaking" => Ok(ChangeCategory::Breaking),
        "deprecation" | "deprecate" => Ok(ChangeCategory::Deprecation),
        "chore" => Ok(ChangeCategory::Chore),
        _ => anyhow::bail!("Unknown category: {}", s),
    }
}

/// Check if a symbol is public based on language conventions
fn is_public_symbol(symbol: &agentjj::symbols::Symbol, lang: agentjj::SupportedLanguage) -> bool {
    match lang {
        agentjj::SupportedLanguage::Rust => {
            // Rust: check for "pub" keyword in signature
            symbol
                .signature
                .as_ref()
                .map(|sig: &String| sig.contains("pub"))
                .unwrap_or(false)
        }
        agentjj::SupportedLanguage::Python => {
            // Python: underscore prefix means private (convention)
            !symbol.name.starts_with('_')
        }
        agentjj::SupportedLanguage::JavaScript | agentjj::SupportedLanguage::TypeScript => {
            // JS/TS: check for "export" keyword in signature
            symbol
                .signature
                .as_ref()
                .map(|sig: &String| sig.contains("export"))
                .unwrap_or(true)
        }
    }
}

fn cmd_context(path: String, json: bool) -> Result<()> {
    // Parse path: "path/to/file.ext::symbol_name"
    let (file_path, symbol_name) = if let Some(idx) = path.find("::") {
        (&path[..idx], &path[idx + 2..])
    } else {
        anyhow::bail!("Symbol path must be path/to/file::symbol_name (e.g., src/main.rs::main)");
    };

    let file_path_obj = std::path::Path::new(file_path);

    // Detect language
    let lang = agentjj::SupportedLanguage::from_path(file_path_obj)
        .ok_or_else(|| anyhow::anyhow!("Unsupported file type: {}", file_path))?;

    // Read file content
    let content = if file_path_obj.is_absolute() {
        std::fs::read_to_string(file_path)?
    } else {
        let mut repo = Repo::discover()?;
        repo.read_file(file_path, None)?
    };

    // Get minimal context
    let context = agentjj::symbols::get_symbol_context(&content, lang, symbol_name)?;

    match context {
        Some(ctx) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&ctx)?);
            } else {
                println!("# {}", ctx.name);
                println!("kind: {:?}", ctx.kind);
                if let Some(sig) = &ctx.signature {
                    println!("\n```");
                    println!("{}", sig);
                    println!("```");
                }
                if let Some(doc) = &ctx.docstring {
                    println!("\n{}", doc);
                }
                if !ctx.imports_needed.is_empty() {
                    println!("\nimports needed:");
                    for imp in &ctx.imports_needed {
                        println!("  {}", imp);
                    }
                }
            }
        }
        None => {
            if json {
                println!(
                    r#"{{"error": "symbol not found", "name": "{}"}}"#,
                    symbol_name
                );
            } else {
                println!("Symbol '{}' not found in {}", symbol_name, file_path);
            }
            std::process::exit(1);
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_commit(
    message: String,
    no_new: bool,
    change_type_str: String,
    category_str: Option<String>,
    no_invariants: bool,
    breaking: bool,
    paths: Option<Vec<String>>,
    json: bool,
) -> Result<()> {
    let mut repo = Repo::discover()?;

    let change_type = parse_change_type(&change_type_str)?;
    let category = match category_str {
        Some(ref c) => Some(parse_category(c)?),
        None => None,
    };

    let opts = agentjj::repo::CommitOptions {
        message: message.clone(),
        no_new,
        run_invariants: !no_invariants,
        change_type,
        category,
        breaking,
        paths,
    };

    let result = repo.commit_working_copy(opts)?;

    if json {
        let invariant_map: serde_json::Value = result
            .invariants
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    serde_json::to_value(v).unwrap_or(serde_json::json!("unknown")),
                )
            })
            .collect::<serde_json::Map<String, serde_json::Value>>()
            .into();

        let output = serde_json::json!({
            "committed": true,
            "change_id": result.change_id,
            "commit": result.commit_id,
            "message": message,
            "files_changed": result.files_changed,
            "invariants": invariant_map,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Committed: {}", message);
        println!("  Change:  {}", result.change_id);
        println!("  Commit:  {}", result.commit_id);
        if !result.files_changed.is_empty() {
            println!("  Files:   {}", result.files_changed.len());
            for f in &result.files_changed {
                println!("    {}", f);
            }
        }
        if !result.invariants.is_empty() {
            println!("  Invariants:");
            for (name, status) in &result.invariants {
                println!("    {}: {:?}", name, status);
            }
        }
    }

    Ok(())
}

fn cmd_tag(
    name: String,
    message: Option<String>,
    force: bool,
    push: bool,
    json: bool,
) -> Result<()> {
    let repo = Repo::discover()?;

    // Build tag command
    let mut args = vec!["tag".to_string()];

    if force {
        args.push("-f".to_string());
    }

    if let Some(ref msg) = message {
        args.push("-a".to_string());
        args.push("-m".to_string());
        args.push(msg.clone());
    }

    args.push(name.clone());

    // Create the tag
    let tag_output = std::process::Command::new("git")
        .current_dir(repo.root())
        .args(&args)
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run git tag: {}", e))?;

    if !tag_output.status.success() {
        let stderr = String::from_utf8_lossy(&tag_output.stderr);
        anyhow::bail!("Failed to create tag: {}", stderr);
    }

    // Push tag if requested
    if push {
        let mut push_args = vec!["push".to_string(), "origin".to_string()];
        if force {
            push_args.push("--force".to_string());
        }
        push_args.push(name.clone());

        let push_output = std::process::Command::new("git")
            .current_dir(repo.root())
            .args(&push_args)
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to push tag: {}", e))?;

        if !push_output.status.success() {
            let stderr = String::from_utf8_lossy(&push_output.stderr);
            anyhow::bail!("Failed to push tag: {}", stderr);
        }
    }

    if json {
        let result = serde_json::json!({
            "tag": name,
            "pushed": push,
            "forced": force,
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else if push {
        println!("✓ Tagged and pushed: {}", name);
    } else {
        println!("✓ Tagged: {}", name);
    }

    Ok(())
}

fn cmd_push(
    branch: Option<String>,
    _change: Option<String>,
    create_pr: bool,
    title: Option<String>,
    body: Option<String>,
    target: String,
    json: bool,
) -> Result<()> {
    let repo = Repo::discover()?;

    // Use git directly for colocated repos (which is our primary mode)
    let branch_name = branch.unwrap_or_else(|| "main".to_string());

    // Get the commit to push (HEAD in git terms)
    let rev_parse = std::process::Command::new("git")
        .current_dir(repo.root())
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run git: {}", e))?;

    if !rev_parse.status.success() {
        anyhow::bail!("Not a git repository or no commits");
    }

    let _commit_sha = String::from_utf8_lossy(&rev_parse.stdout)
        .trim()
        .to_string();

    // Push to remote using git
    let push_output = std::process::Command::new("git")
        .current_dir(repo.root())
        .args(["push", "origin", &format!("HEAD:{}", branch_name)])
        .output()?;

    if !push_output.status.success() {
        let stderr = String::from_utf8_lossy(&push_output.stderr);
        anyhow::bail!("Push failed: {}", stderr);
    }

    let mut result = serde_json::json!({
        "pushed": true,
        "branch": branch_name,
    });

    if !json {
        println!("✓ Pushed to {}", branch_name);
    }

    // Create PR if requested
    if create_pr {
        let pr_title = title.ok_or_else(|| anyhow::anyhow!("--title required for PR creation"))?;

        let mut gh_args = vec![
            "pr".to_string(),
            "create".to_string(),
            "--head".to_string(),
            branch_name.clone(),
            "--base".to_string(),
            target.clone(),
            "--title".to_string(),
            pr_title.clone(),
        ];

        if let Some(b) = &body {
            gh_args.push("--body".to_string());
            gh_args.push(b.clone());
        }

        let pr_output = std::process::Command::new("gh")
            .current_dir(repo.root())
            .args(&gh_args)
            .output()?;

        if pr_output.status.success() {
            let pr_url = String::from_utf8_lossy(&pr_output.stdout)
                .trim()
                .to_string();
            result["pr_created"] = serde_json::json!(true);
            result["pr_url"] = serde_json::json!(pr_url);

            if !json {
                println!("✓ Created PR: {}", pr_url);
            }
        } else {
            let stderr = String::from_utf8_lossy(&pr_output.stderr);
            result["pr_created"] = serde_json::json!(false);
            result["pr_error"] = serde_json::json!(stderr.to_string());

            if !json {
                println!("✗ Failed to create PR: {}", stderr);
            }
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    }

    Ok(())
}

/// Complete repository orientation - everything an agent needs to start working
fn cmd_orient(json: bool) -> Result<()> {
    let mut repo = Repo::discover()?;

    let change_id = repo
        .current_change_id()
        .unwrap_or_else(|_| "unknown".into());
    let operation_id = repo
        .current_operation_id()
        .unwrap_or_else(|_| "unknown".into());
    let files = repo.changed_files(&change_id).unwrap_or_default();
    let has_manifest = repo.has_manifest();

    // Get manifest info
    let manifest_info = if has_manifest {
        repo.manifest().ok().map(|m| {
            serde_json::json!({
                "name": m.repo.name,
                "description": m.repo.description,
                "languages": m.repo.languages,
                "invariants_count": m.invariants.len(),
                "permissions": {
                    "allow": m.permissions.allow_change,
                    "deny": m.permissions.deny_change,
                },
            })
        })
    } else {
        None
    };

    // Count files by extension
    let mut file_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut total_files = 0;

    // Patterns to exclude from file counting
    let exclude_patterns = [
        ".jj",
        ".git",
        "target/",
        "node_modules/",
        ".agent/",
        "__pycache__",
        ".pyc",
        "venv/",
        ".venv/",
    ];

    if let Ok(entries) = glob::glob(&format!("{}/**/*", repo.root().display())) {
        for entry in entries.flatten() {
            let path_str = entry.to_string_lossy();
            let should_exclude = exclude_patterns.iter().any(|p| path_str.contains(p));

            if entry.is_file() && !should_exclude {
                total_files += 1;
                if let Some(ext) = entry.extension() {
                    *file_counts
                        .entry(ext.to_string_lossy().to_string())
                        .or_insert(0) += 1;
                }
            }
        }
    }

    // Get recent changes via jj-lib (no jj CLI dependency)
    let recent_changes: Vec<serde_json::Value> = repo
        .log_entries(5, false)
        .unwrap_or_default()
        .into_iter()
        .map(|entry| {
            serde_json::json!({
                "change_id": entry.change_id,
                "description": if entry.description.is_empty() {
                    "(no description)".to_string()
                } else {
                    entry.description
                },
            })
        })
        .collect();

    // Get typed changes
    let typed_changes = agentjj::change::ChangeIndex::load_from_repo(repo.root())
        .ok()
        .map(|idx| idx.all().len())
        .unwrap_or(0);

    let orientation = serde_json::json!({
        "current_state": {
            "change_id": change_id,
            "operation_id": &operation_id[..32.min(operation_id.len())],
            "uncommitted_files": files,
        },
        "repository": manifest_info,
        "codebase": {
            "total_files": total_files,
            "by_extension": file_counts,
            "typed_changes": typed_changes,
        },
        "recent_changes": recent_changes,
        "capabilities": {
            "symbol_query": ["python", "rust", "javascript", "typescript"],
            "commands": [
                "status", "read", "symbol", "context", "apply",
                "change", "push", "orient", "checkpoint", "undo",
                "bulk", "files", "diff", "affected", "validate", "suggest"
            ],
        },
        "quick_start": {
            "read_file": "agentjj read <path>",
            "query_symbol": "agentjj symbol <file>::<name>",
            "get_context": "agentjj context <file>::<name>",
            "make_change": "agentjj apply --intent '...' --type behavioral --patch <file>",
            "save_checkpoint": "agentjj checkpoint <name>",
        },
    });

    if json {
        println!("{}", serde_json::to_string_pretty(&orientation)?);
    } else {
        println!("=== Repository Orientation ===\n");
        println!("Current change: {}", &change_id[..12.min(change_id.len())]);
        if !files.is_empty() {
            println!("Uncommitted: {} files", files.len());
        }
        println!();

        if let Some(info) = &manifest_info {
            println!("Project: {}", info["name"]);
            if !info["description"].as_str().unwrap_or("").is_empty() {
                println!("  {}", info["description"]);
            }
        }

        println!("\nCodebase: {} files", total_files);
        let mut sorted_counts: Vec<_> = file_counts.iter().collect();
        sorted_counts.sort_by(|a, b| b.1.cmp(a.1));
        for (ext, count) in sorted_counts.iter().take(5) {
            println!("  .{}: {}", ext, count);
        }

        if !recent_changes.is_empty() {
            println!("\nRecent changes:");
            for c in recent_changes.iter().take(3) {
                let cid = c["change_id"].as_str().unwrap_or("");
                let short_id = &cid[..8.min(cid.len())];
                println!("  {} {}", short_id, c["description"]);
            }
        }

        println!("\n=== Quick Start ===");
        println!("  agentjj symbol <file>           # List symbols in file");
        println!("  agentjj context <file>::<name>  # Get symbol context");
        println!("  agentjj bulk read <files...>    # Read multiple files");
        println!("  agentjj checkpoint create <name> # Save restore point");
    }

    Ok(())
}

/// Create a named checkpoint
fn cmd_checkpoint(name: String, description: Option<String>, json: bool) -> Result<()> {
    let mut repo = Repo::discover()?;

    let change_id = repo.current_change_id()?;
    let operation_id = repo.current_operation_id()?;

    // Store checkpoint as a file in .agent/checkpoints/
    let checkpoints_dir = repo.root().join(".agent/checkpoints");
    std::fs::create_dir_all(&checkpoints_dir)?;

    let checkpoint = serde_json::json!({
        "name": name,
        "description": description,
        "change_id": change_id,
        "operation_id": operation_id,
        "created_at": chrono_lite_now(),
    });

    let checkpoint_path = checkpoints_dir.join(format!("{}.json", name));
    std::fs::write(&checkpoint_path, serde_json::to_string_pretty(&checkpoint)?)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "created": true,
                "checkpoint": checkpoint,
                "restore_command": format!("agentjj undo --to {}", name),
            }))?
        );
    } else {
        println!("✓ Checkpoint '{}' created", name);
        println!("  change: {}", &change_id[..12.min(change_id.len())]);
        println!("  restore with: agentjj undo --to {}", name);
    }

    Ok(())
}

/// List all checkpoints sorted by created_at descending
fn cmd_checkpoint_list(json: bool) -> Result<()> {
    let repo = Repo::discover()?;
    let checkpoints_dir = repo.root().join(".agent/checkpoints");

    if !checkpoints_dir.exists() || !checkpoints_dir.is_dir() {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "checkpoints": []
                }))?
            );
        } else {
            println!("No checkpoints found.");
        }
        return Ok(());
    }

    let mut checkpoints: Vec<serde_json::Value> = Vec::new();

    for entry in std::fs::read_dir(&checkpoints_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            if let Ok(checkpoint) = serde_json::from_str::<serde_json::Value>(&content) {
                checkpoints.push(checkpoint);
            }
        }
    }

    if checkpoints.is_empty() {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "checkpoints": []
                }))?
            );
        } else {
            println!("No checkpoints found.");
        }
        return Ok(());
    }

    // Sort by created_at descending
    checkpoints.sort_by(|a, b| {
        let a_time = a["created_at"].as_str().unwrap_or("");
        let b_time = b["created_at"].as_str().unwrap_or("");
        b_time.cmp(a_time)
    });

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "checkpoints": checkpoints
            }))?
        );
    } else {
        println!("Checkpoints:");
        for cp in &checkpoints {
            let name = cp["name"].as_str().unwrap_or("(unknown)");
            let created_at = cp["created_at"].as_str().unwrap_or("");
            // Format timestamp for display: "2026-02-14T10:23:15Z" -> "2026-02-14 10:23:15"
            let display_time = created_at
                .replace('T', " ")
                .trim_end_matches('Z')
                .to_string();
            let description = cp["description"]
                .as_str()
                .map(|d| format!("\"{}\"", d))
                .unwrap_or_else(|| "(no description)".to_string());
            println!("  {:<30} {}  {}", name, display_time, description);
        }
    }

    Ok(())
}

fn chrono_lite_now() -> String {
    // Simple ISO 8601 timestamp without chrono dependency
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();

    // Convert unix timestamp to ISO 8601 format
    // Algorithm: days since epoch -> year/month/day, seconds -> hours:minutes:seconds
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // Calculate year/month/day from days since 1970-01-01
    let mut year = 1970;
    let mut remaining_days = days as i64;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let is_leap = is_leap_year(year);
    let days_in_months: [i64; 12] = if is_leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1;
    for days_in_month in days_in_months.iter() {
        if remaining_days < *days_in_month {
            break;
        }
        remaining_days -= *days_in_month;
        month += 1;
    }
    let day = remaining_days + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Undo operations or restore to checkpoint
fn cmd_undo(steps: usize, to: Option<String>, dry_run: bool, json: bool) -> Result<()> {
    let mut repo = Repo::discover()?;

    // If --to is specified, restore to named checkpoint
    if let Some(checkpoint_name) = to {
        let checkpoint_path = repo
            .root()
            .join(".agent/checkpoints")
            .join(format!("{}.json", checkpoint_name));

        if !checkpoint_path.exists() {
            anyhow::bail!("Checkpoint '{}' not found", checkpoint_name);
        }

        let checkpoint_data: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&checkpoint_path)?)?;
        let target_op = checkpoint_data["operation_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid checkpoint: missing operation_id"))?;

        if dry_run {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "dry_run": true,
                        "checkpoint": checkpoint_name,
                        "would_restore_to": target_op,
                        "checkpoint_data": checkpoint_data,
                    })
                );
            } else {
                println!("Would restore to checkpoint '{}'", checkpoint_name);
                println!(
                    "Would restore to operation: {}...",
                    &target_op[..16.min(target_op.len())]
                );
            }
            return Ok(());
        }

        // Restore to checkpoint operation using Repo method
        repo.restore_operation(target_op)?;

        if json {
            println!(
                "{}",
                serde_json::json!({
                    "restored": true,
                    "checkpoint": checkpoint_name,
                    "restored_to": target_op,
                })
            );
        } else {
            println!("✓ Restored to checkpoint '{}'", checkpoint_name);
        }

        return Ok(());
    }

    // Otherwise, undo by steps
    // Use Repo.operation_log() to find operations to undo
    let operations = repo.operation_log(steps + 1)?;

    if operations.len() <= steps {
        anyhow::bail!("Not enough operations to undo {} steps", steps);
    }

    let target_op = &operations[steps].id;

    if dry_run {
        if json {
            println!(
                "{}",
                serde_json::json!({
                    "dry_run": true,
                    "would_restore_to": target_op,
                    "operations_to_undo": steps,
                })
            );
        } else {
            println!("Would undo {} operation(s)", steps);
            println!(
                "Would restore to operation: {}...",
                &target_op[..16.min(target_op.len())]
            );
        }
        return Ok(());
    }

    // Actually undo using Repo method
    repo.restore_operation(target_op)?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "undone": true,
                "steps": steps,
                "restored_to": target_op,
            })
        );
    } else {
        println!("✓ Undid {} operation(s)", steps);
    }

    Ok(())
}

/// Bulk operations
fn cmd_bulk(action: BulkAction, json: bool) -> Result<()> {
    let mut repo = Repo::discover()?;

    match action {
        BulkAction::Read { paths } => {
            let mut results = Vec::new();
            let mut errors = Vec::new();

            for path in &paths {
                match repo.read_file(path, None) {
                    Ok(content) => {
                        results.push(serde_json::json!({
                            "path": path,
                            "content": content,
                            "lines": content.lines().count(),
                        }));
                    }
                    Err(e) => {
                        errors.push(serde_json::json!({
                            "path": path,
                            "error": e.to_string(),
                        }));
                    }
                }
            }

            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "files": results,
                        "errors": errors,
                        "summary": {
                            "read": results.len(),
                            "failed": errors.len(),
                        }
                    }))?
                );
            } else {
                for r in &results {
                    println!("=== {} ({} lines) ===", r["path"], r["lines"]);
                    println!("{}", r["content"].as_str().unwrap_or(""));
                    println!();
                }
                for e in &errors {
                    eprintln!("Error reading {}: {}", e["path"], e["error"]);
                }
            }
        }

        BulkAction::Symbols {
            pattern,
            public_only,
        } => {
            let mut all_symbols = Vec::new();

            // Use glob to find matching files
            let glob_pattern = format!("{}/{}", repo.root().display(), pattern);
            if let Ok(entries) = glob::glob(&glob_pattern) {
                for entry in entries.flatten() {
                    if entry.is_file() {
                        if let Some(lang) = agentjj::SupportedLanguage::from_path(&entry) {
                            if let Ok(content) = std::fs::read_to_string(&entry) {
                                if let Ok(symbols) =
                                    agentjj::symbols::extract_symbols(&content, lang)
                                {
                                    let rel_path =
                                        entry.strip_prefix(repo.root()).unwrap_or(&entry);
                                    for s in symbols {
                                        if !public_only || is_public_symbol(&s, lang) {
                                            all_symbols.push(serde_json::json!({
                                                "file": rel_path.display().to_string(),
                                                "name": s.name,
                                                "kind": s.kind,
                                                "line": s.start_line,
                                                "signature": s.signature,
                                            }));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "pattern": pattern,
                        "symbols": all_symbols,
                        "count": all_symbols.len(),
                    }))?
                );
            } else {
                println!(
                    "Found {} symbols matching '{}':",
                    all_symbols.len(),
                    pattern
                );
                for s in &all_symbols {
                    println!(
                        "  {}::{} ({:?}, line {})",
                        s["file"], s["name"], s["kind"], s["line"]
                    );
                }
            }
        }

        BulkAction::Context { symbols } => {
            let mut results = Vec::new();
            let mut errors = Vec::new();

            for sym_path in &symbols {
                if let Some(idx) = sym_path.find("::") {
                    let (file_path, symbol_name) = (&sym_path[..idx], &sym_path[idx + 2..]);
                    let file_path_obj = std::path::Path::new(file_path);

                    if let Some(lang) = agentjj::SupportedLanguage::from_path(file_path_obj) {
                        let content_result: Result<String> = if file_path_obj.is_absolute() {
                            std::fs::read_to_string(file_path).map_err(|e| anyhow::anyhow!("{}", e))
                        } else {
                            repo.read_file(file_path, None)
                                .map_err(|e| anyhow::anyhow!("{}", e))
                        };

                        match content_result {
                            Ok(content) => {
                                match agentjj::symbols::get_symbol_context(
                                    &content,
                                    lang,
                                    symbol_name,
                                ) {
                                    Ok(Some(ctx)) => {
                                        results.push(serde_json::json!({
                                            "path": sym_path,
                                            "context": ctx,
                                        }));
                                    }
                                    Ok(None) => {
                                        errors.push(serde_json::json!({
                                            "path": sym_path,
                                            "error": "symbol not found",
                                        }));
                                    }
                                    Err(e) => {
                                        errors.push(serde_json::json!({
                                            "path": sym_path,
                                            "error": e.to_string(),
                                        }));
                                    }
                                }
                            }
                            Err(e) => {
                                errors.push(serde_json::json!({
                                    "path": sym_path,
                                    "error": e.to_string(),
                                }));
                            }
                        }
                    } else {
                        errors.push(serde_json::json!({
                            "path": sym_path,
                            "error": "unsupported file type",
                        }));
                    }
                } else {
                    errors.push(serde_json::json!({
                        "path": sym_path,
                        "error": "invalid format, expected file::symbol",
                    }));
                }
            }

            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "contexts": results,
                        "errors": errors,
                    }))?
                );
            } else {
                for r in &results {
                    println!("=== {} ===", r["path"]);
                    let ctx = &r["context"];
                    println!("  {} ({:?})", ctx["name"], ctx["kind"]);
                    if let Some(sig) = ctx["signature"].as_str() {
                        println!("  {}", sig);
                    }
                    println!();
                }
            }
        }
    }

    Ok(())
}

/// List files with optional symbol counts
fn cmd_files(pattern: Option<String>, with_symbols: bool, json: bool) -> Result<()> {
    let repo = Repo::discover()?;

    let glob_pattern = pattern.unwrap_or_else(|| "**/*".to_string());
    let full_pattern = format!("{}/{}", repo.root().display(), glob_pattern);

    let mut files = Vec::new();

    if let Ok(entries) = glob::glob(&full_pattern) {
        for entry in entries.flatten() {
            if entry.is_file()
                && !entry.to_string_lossy().contains(".jj")
                && !entry.to_string_lossy().contains(".git")
            {
                let rel_path = entry.strip_prefix(repo.root()).unwrap_or(&entry);
                let ext = entry.extension().map(|e| e.to_string_lossy().to_string());
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);

                let mut file_info = serde_json::json!({
                    "path": rel_path.display().to_string(),
                    "extension": ext,
                    "size": size,
                });

                if with_symbols {
                    if let Some(lang) = agentjj::SupportedLanguage::from_path(&entry) {
                        if let Ok(content) = std::fs::read_to_string(&entry) {
                            if let Ok(symbols) = agentjj::symbols::extract_symbols(&content, lang) {
                                file_info["symbol_count"] = serde_json::json!(symbols.len());
                                file_info["symbols"] = serde_json::json!(symbols
                                    .iter()
                                    .map(|s| &s.name)
                                    .collect::<Vec<_>>());
                            }
                        }
                    }
                }

                files.push(file_info);
            }
        }
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "pattern": glob_pattern,
                "files": files,
                "count": files.len(),
            }))?
        );
    } else {
        println!("Files matching '{}':", glob_pattern);
        for f in &files {
            let size_str = format_size(f["size"].as_u64().unwrap_or(0));
            if with_symbols {
                if let Some(count) = f["symbol_count"].as_u64() {
                    println!("  {} ({}, {} symbols)", f["path"], size_str, count);
                } else {
                    println!("  {} ({})", f["path"], size_str);
                }
            } else {
                println!("  {} ({})", f["path"], size_str);
            }
        }
        println!("\nTotal: {} files", files.len());
    }

    Ok(())
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Show semantic diff
fn cmd_diff(against: Option<String>, explain: bool, json: bool) -> Result<()> {
    let mut repo = Repo::discover()?;
    let target = against.unwrap_or_else(|| "@-".to_string());

    // agentjj is colocated with git; use git for diff rendering since jj CLI
    // is not required to be installed.
    let diff_output = if target == "@" {
        // Working copy changes: compare git HEAD to working tree
        std::process::Command::new("git")
            .current_dir(repo.root())
            .args(["diff", "HEAD"])
            .output()?
    } else {
        // Resolve the jj revision to git-compatible commit IDs.
        // In colocated mode, jj commit IDs are git commit IDs.
        let (parent_hex, commit_hex) = repo.resolve_revision(&target)?;

        match parent_hex {
            Some(parent) => std::process::Command::new("git")
                .current_dir(repo.root())
                .args(["diff", &parent, &commit_hex])
                .output()?,
            None => {
                // Root commit: show entire commit as additions
                std::process::Command::new("git")
                    .current_dir(repo.root())
                    .args(["show", "--format=", &commit_hex])
                    .output()?
            }
        }
    };

    if !diff_output.status.success() {
        let stderr = String::from_utf8_lossy(&diff_output.stderr);
        anyhow::bail!("Diff failed: {}", stderr);
    }

    let raw_diff = String::from_utf8_lossy(&diff_output.stdout).to_string();

    // Parse diff into structured format
    let mut files_changed = Vec::new();
    let mut current_file: Option<String> = None;
    let mut additions = 0;
    let mut deletions = 0;

    for line in raw_diff.lines() {
        if line.starts_with("--- ") || line.starts_with("+++ ") {
            let path = line[4..].trim_start_matches("a/").trim_start_matches("b/");
            if !path.is_empty() && path != "/dev/null" {
                current_file = Some(path.to_string());
            }
        } else if line.starts_with('+') && !line.starts_with("+++") {
            additions += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            deletions += 1;
        }

        if let Some(ref file) = current_file {
            if !files_changed.contains(file) {
                files_changed.push(file.clone());
            }
        }
    }

    let semantic_summary = if explain && !files_changed.is_empty() {
        // Generate a semantic summary based on file types and changes
        let mut summary_parts = Vec::new();

        for file in &files_changed {
            let ext = std::path::Path::new(file)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");

            let file_type = match ext {
                "rs" => "Rust code",
                "py" => "Python code",
                "ts" | "tsx" => "TypeScript code",
                "js" | "jsx" => "JavaScript code",
                "toml" => "TOML configuration",
                "json" => "JSON data",
                "md" => "documentation",
                "yaml" | "yml" => "YAML configuration",
                _ => "file",
            };

            summary_parts.push(format!("{} ({})", file, file_type));
        }

        Some(format!(
            "Changes affect {} file(s): {}. Net change: +{} -{} lines.",
            files_changed.len(),
            summary_parts.join(", "),
            additions,
            deletions
        ))
    } else {
        None
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "against": target,
                "files_changed": files_changed,
                "stats": {
                    "additions": additions,
                    "deletions": deletions,
                    "net": additions as i64 - deletions as i64,
                },
                "explanation": semantic_summary,
                "raw_diff": raw_diff,
            }))?
        );
    } else {
        println!("Diff against {}:", target);
        println!("  {} file(s) changed", files_changed.len());
        println!("  +{} -{} lines", additions, deletions);

        if let Some(summary) = &semantic_summary {
            println!("\nSummary: {}", summary);
        }

        println!("\n{}", raw_diff);
    }

    Ok(())
}

/// Analyze what would be affected by changing a symbol
fn cmd_affected(symbol_path: String, depth: usize, json: bool) -> Result<()> {
    let repo = Repo::discover()?;

    // Parse the symbol path
    let (file_path, symbol_name) = if let Some(idx) = symbol_path.find("::") {
        (&symbol_path[..idx], &symbol_path[idx + 2..])
    } else {
        anyhow::bail!("Symbol path must be file::symbol_name");
    };

    // Find all files that might reference this symbol
    let mut affected_files = Vec::new();
    let pattern = format!("{}/**/*", repo.root().display());

    if let Ok(entries) = glob::glob(&pattern) {
        for entry in entries.flatten() {
            if entry.is_file() {
                if let Some(lang) = agentjj::SupportedLanguage::from_path(&entry) {
                    if let Ok(content) = std::fs::read_to_string(&entry) {
                        // Simple text search for the symbol name
                        if content.contains(symbol_name) {
                            let rel_path = entry.strip_prefix(repo.root()).unwrap_or(&entry);

                            // Count occurrences
                            let occurrences = content.matches(symbol_name).count();

                            // Try to find actual usages (not just the definition)
                            let is_definition = rel_path.to_string_lossy() == file_path;

                            if !is_definition || depth > 0 {
                                affected_files.push(serde_json::json!({
                                    "path": rel_path.display().to_string(),
                                    "language": format!("{:?}", lang),
                                    "occurrences": occurrences,
                                    "is_definition": is_definition,
                                }));
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort by occurrences (most affected first)
    affected_files.sort_by(|a, b| {
        b["occurrences"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["occurrences"].as_u64().unwrap_or(0))
    });

    let analysis = serde_json::json!({
        "symbol": symbol_path,
        "depth": depth,
        "affected_files": affected_files,
        "total_files": affected_files.len(),
        "risk_assessment": if affected_files.len() > 10 {
            "high"
        } else if affected_files.len() > 3 {
            "medium"
        } else {
            "low"
        },
        "recommendation": if affected_files.len() > 10 {
            "Consider creating a deprecation path or using feature flags"
        } else if affected_files.len() > 3 {
            "Run tests after change, review affected files"
        } else {
            "Safe to modify with standard review"
        },
    });

    if json {
        println!("{}", serde_json::to_string_pretty(&analysis)?);
    } else {
        println!("Impact analysis for '{}':", symbol_path);
        println!("  Risk: {}", analysis["risk_assessment"]);
        println!("  {} file(s) affected", affected_files.len());
        println!();

        for f in affected_files.iter().take(10) {
            let marker = if f["is_definition"].as_bool().unwrap_or(false) {
                "(def)"
            } else {
                ""
            };
            println!("  {} ({} refs) {}", f["path"], f["occurrences"], marker);
        }

        if affected_files.len() > 10 {
            println!("  ... and {} more", affected_files.len() - 10);
        }

        println!("\n{}", analysis["recommendation"]);
    }

    Ok(())
}

/// Print JSON schemas for output types
fn cmd_schema(type_filter: Option<String>, json: bool) -> Result<()> {
    let schemas = serde_json::json!({
        "status": {
            "type": "object",
            "properties": {
                "change_id": { "type": "string", "description": "Current jj change ID" },
                "operation_id": { "type": "string", "description": "Current jj operation ID" },
                "files_changed": { "type": "array", "items": { "type": "string" } },
                "has_manifest": { "type": "boolean" },
                "typed_change": { "type": "object", "nullable": true },
            }
        },
        "symbol": {
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "kind": { "type": "string", "enum": ["function", "method", "class", "struct", "enum", "interface", "constant", "variable", "module", "import"] },
                "signature": { "type": "string", "nullable": true },
                "docstring": { "type": "string", "nullable": true },
                "start_line": { "type": "integer" },
                "end_line": { "type": "integer" },
            }
        },
        "context": {
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "kind": { "type": "string" },
                "signature": { "type": "string", "nullable": true },
                "docstring": { "type": "string", "nullable": true },
                "imports_needed": { "type": "array", "items": { "type": "string" } },
            }
        },
        "apply_result": {
            "oneOf": [
                {
                    "type": "object",
                    "properties": {
                        "status": { "const": "success" },
                        "change_id": { "type": "string" },
                        "files_changed": { "type": "array" },
                    }
                },
                {
                    "type": "object",
                    "properties": {
                        "status": { "const": "precondition_failed" },
                        "reason": { "type": "string" },
                        "expected": { "type": "string" },
                        "actual": { "type": "string" },
                    }
                },
                {
                    "type": "object",
                    "properties": {
                        "status": { "const": "conflict" },
                        "conflicts": { "type": "array" },
                    }
                }
            ]
        },
        "error": {
            "type": "object",
            "properties": {
                "error": { "const": true },
                "message": { "type": "string" },
            }
        },
        "orient": {
            "type": "object",
            "description": "Complete repository orientation for agents",
            "properties": {
                "current_state": { "type": "object" },
                "repository": { "type": "object", "nullable": true },
                "codebase": { "type": "object" },
                "recent_changes": { "type": "array" },
                "capabilities": { "type": "object" },
                "quick_start": { "type": "object" },
            }
        },
    });

    if let Some(type_name) = type_filter {
        if let Some(schema) = schemas.get(&type_name) {
            if json {
                println!("{}", serde_json::to_string_pretty(schema)?);
            } else {
                println!("Schema for '{}':", type_name);
                println!("{}", serde_json::to_string_pretty(schema)?);
            }
        } else {
            anyhow::bail!(
                "Unknown type: {}. Available: status, symbol, context, apply_result, error, orient",
                type_name
            );
        }
    } else if json {
        println!("{}", serde_json::to_string_pretty(&schemas)?);
    } else {
        println!("Available schemas:");
        for key in schemas.as_object().unwrap().keys() {
            println!("  {}", key);
        }
        println!("\nUse --type <name> to see a specific schema");
    }

    Ok(())
}

/// Validate current changes are complete
fn cmd_validate(json: bool) -> Result<()> {
    let mut repo = Repo::discover()?;

    let change_id = repo.current_change_id()?;
    let files = repo.changed_files(&change_id)?;

    let mut issues = Vec::new();
    let mut warnings = Vec::new();

    // Check if there are any changes
    if files.is_empty() {
        issues.push("No changes to validate".to_string());
    }

    // Check for typed change metadata
    let typed_change = repo.get_typed_change(&change_id).ok();
    if typed_change.is_none() {
        warnings.push("No typed change metadata - consider using 'agentjj change set'".to_string());
    }

    // Check manifest exists
    if !repo.has_manifest() {
        warnings.push("No manifest found - consider using 'agentjj init'".to_string());
    }

    // Check for common issues in changed files
    for file in &files {
        let path = std::path::Path::new(file);

        // Check for test files if code was changed
        if path
            .extension()
            .map(|e| e == "rs" || e == "py" || e == "ts" || e == "js")
            .unwrap_or(false)
        {
            let is_test = file.contains("test")
                || file.contains("spec")
                || file.contains("_test.")
                || file.contains(".test.");
            if !is_test {
                // For Rust files, tests are often inline (mod tests) - skip warning
                // For other languages, check common test locations
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext != "rs" {
                    let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

                    // Check common test locations
                    let test_patterns = [
                        format!("tests/{}.{}", file_stem, ext),
                        format!("test/{}.{}", file_stem, ext),
                        format!("tests/test_{}.{}", file_stem, ext),
                        format!("{}_test.{}", file_stem, ext),
                        format!("{}.test.{}", file_stem, ext),
                        format!("{}.spec.{}", file_stem, ext),
                    ];

                    let has_test = test_patterns.iter().any(|p| repo.root().join(p).exists());
                    if !has_test {
                        warnings.push(format!("Consider adding tests for {}", file));
                    }
                }
            }
        }
    }

    // Check invariants from manifest
    if let Ok(manifest) = repo.manifest() {
        if !manifest.invariants.is_empty() {
            warnings.push(format!(
                "{} invariant(s) defined - run tests manually to verify",
                manifest.invariants.len()
            ));
        }
    }

    let is_valid = issues.is_empty();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "valid": is_valid,
                "change_id": change_id,
                "files_changed": files,
                "typed_change": typed_change,
                "issues": issues,
                "warnings": warnings,
            }))?
        );
    } else {
        if is_valid {
            println!("✓ Changes are valid");
        } else {
            println!("✗ Validation failed");
        }

        println!("  {} file(s) changed", files.len());

        if !issues.is_empty() {
            println!("\nIssues:");
            for issue in &issues {
                println!("  ✗ {}", issue);
            }
        }

        if !warnings.is_empty() {
            println!("\nWarnings:");
            for warning in &warnings {
                println!("  ⚠ {}", warning);
            }
        }

        if is_valid && warnings.is_empty() {
            println!("\nReady to push!");
        }
    }

    if !is_valid {
        std::process::exit(1);
    }

    Ok(())
}

/// Output the repository DAG in various formats
fn cmd_graph(format: String, limit: usize, all: bool, json: bool) -> Result<()> {
    let mut repo = Repo::discover()?;

    match format.to_lowercase().as_str() {
        "ascii" => cmd_graph_ascii(&mut repo, limit, all, json),
        "mermaid" => cmd_graph_mermaid(&mut repo, limit, all, json),
        "dot" => cmd_graph_dot(&mut repo, limit, all, json),
        _ => anyhow::bail!(
            "Unknown format: {}. Use 'ascii', 'mermaid', or 'dot'",
            format
        ),
    }
}

/// Graph node representation for structured output
#[derive(Clone)]
struct GraphNode {
    id: String,
    description: String,
    parents: Vec<String>,
    timestamp: Option<String>,
    author: Option<String>,
    full_commit_id: String,
}

/// Get structured graph nodes using Repo.log_entries()
fn get_graph_nodes(repo: &mut Repo, limit: usize, all: bool) -> Result<Vec<GraphNode>> {
    let entries = repo.log_entries(limit, all)?;

    let nodes = entries
        .into_iter()
        .map(|entry| GraphNode {
            id: entry.change_id,
            description: entry.description,
            parents: entry.parent_change_ids,
            timestamp: entry.timestamp,
            author: entry.author,
            full_commit_id: entry.full_commit_id,
        })
        .collect();

    Ok(nodes)
}

/// ASCII format: structured log output with optional timestamps
fn cmd_graph_ascii(repo: &mut Repo, limit: usize, all: bool, json: bool) -> Result<()> {
    let nodes = get_graph_nodes(repo, limit, all)?;

    if json {
        // Also get the raw ASCII diagram for backwards compatibility
        let ascii_output = repo.log_ascii(limit, all).unwrap_or_default();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "format": "ascii",
                "diagram": ascii_output,
                "nodes": nodes.iter().map(|n| serde_json::json!({
                    "id": n.id,
                    "description": n.description,
                    "parents": n.parents,
                    "timestamp": n.timestamp,
                    "author": n.author,
                    "full_commit_id": n.full_commit_id,
                })).collect::<Vec<_>>(),
            }))?
        );
    } else {
        // Render ASCII graph with timestamps inline
        for node in &nodes {
            let ts_part = node
                .timestamp
                .as_deref()
                .map(|ts| format!(" [{}]", ts))
                .unwrap_or_default();
            let desc = if node.description.is_empty() {
                "(empty)".to_string()
            } else {
                node.description.clone()
            };
            println!("* {}{} {}", node.id, ts_part, desc);
        }
    }

    Ok(())
}

/// Mermaid format: generate flowchart from jj log
fn cmd_graph_mermaid(repo: &mut Repo, limit: usize, all: bool, json: bool) -> Result<()> {
    let nodes = get_graph_nodes(repo, limit, all)?;

    // Build Mermaid flowchart
    let mut diagram = String::from("flowchart TD\n");

    for node in &nodes {
        // Escape quotes in description and truncate
        let desc = node.description.replace('"', "'").replace('\n', " ");
        let truncated_desc = if desc.len() > 40 {
            format!("{}...", &desc[..37])
        } else {
            desc.clone()
        };

        // Include timestamp in the node label when available
        let ts_suffix = node
            .timestamp
            .as_deref()
            .map(|ts| format!("<br/>{}", ts))
            .unwrap_or_default();

        // Node definition with short ID
        diagram.push_str(&format!(
            "  {}[\"{}{}\"]\n",
            node.id, truncated_desc, ts_suffix
        ));

        // Edges to parents
        for parent_id in &node.parents {
            diagram.push_str(&format!("  {} --> {}\n", node.id, parent_id));
        }
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "format": "mermaid",
                "diagram": diagram,
                "nodes": nodes.iter().map(|n| serde_json::json!({
                    "id": n.id,
                    "description": n.description,
                    "parents": n.parents,
                    "timestamp": n.timestamp,
                    "author": n.author,
                    "full_commit_id": n.full_commit_id,
                })).collect::<Vec<_>>(),
            }))?
        );
    } else {
        print!("{}", diagram);
    }

    Ok(())
}

/// DOT format: generate Graphviz output from jj log
fn cmd_graph_dot(repo: &mut Repo, limit: usize, all: bool, json: bool) -> Result<()> {
    let nodes = get_graph_nodes(repo, limit, all)?;

    // Build DOT graph
    let mut diagram = String::from("digraph G {\n");
    diagram.push_str("  rankdir=BT;\n");
    diagram.push_str("  node [shape=box, style=rounded];\n\n");

    for node in &nodes {
        // Escape quotes in description and truncate
        let desc = node.description.replace('"', "\\\"").replace('\n', "\\n");
        let truncated_desc = if desc.len() > 40 {
            format!("{}...", &desc[..37])
        } else {
            desc.clone()
        };

        // Include timestamp in the label when available
        let ts_line = node
            .timestamp
            .as_deref()
            .map(|ts| format!("\\n{}", ts))
            .unwrap_or_default();

        // Node definition
        diagram.push_str(&format!(
            "  \"{}\" [label=\"{}\\n{}{}\"];\n",
            node.id, node.id, truncated_desc, ts_line
        ));

        // Edges to parents
        for parent_id in &node.parents {
            diagram.push_str(&format!("  \"{}\" -> \"{}\";\n", node.id, parent_id));
        }
    }

    diagram.push_str("}\n");

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "format": "dot",
                "diagram": diagram,
                "nodes": nodes.iter().map(|n| serde_json::json!({
                    "id": n.id,
                    "description": n.description,
                    "parents": n.parents,
                    "timestamp": n.timestamp,
                    "author": n.author,
                    "full_commit_id": n.full_commit_id,
                })).collect::<Vec<_>>(),
            }))?
        );
    } else {
        print!("{}", diagram);
    }

    Ok(())
}

/// Output the full skill documentation, embedded at compile time
fn cmd_skill(json: bool) -> Result<()> {
    let skill_text = include_str!("../docs/skill.md");

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "format": "markdown",
                "content": skill_text,
                "description": "Full agentjj skill documentation for agent self-discovery",
            }))?
        );
    } else {
        print!("{}", skill_text);
    }

    Ok(())
}

/// Show a concise getting-started guide (works without a repo)
fn cmd_quickstart(json: bool) -> Result<()> {
    let steps = [
        (
            "orient",
            "agentjj orient",
            "Get a complete repo briefing — current state, codebase stats, capabilities",
        ),
        (
            "status",
            "agentjj status",
            "Check working copy changes and current change ID",
        ),
        (
            "checkpoint",
            "agentjj checkpoint <name>",
            "Save a named restore point before making changes",
        ),
        (
            "diff",
            "agentjj diff",
            "Review your changes before committing",
        ),
        (
            "commit",
            "agentjj commit -m \"feat: description\"",
            "Commit with a typed message (describe + new working copy)",
        ),
        (
            "push",
            "agentjj push --branch main",
            "Push changes to the remote",
        ),
    ];

    let tips = [
        "Use --json on any command for machine-parseable output",
        "Run agentjj suggest to get context-aware next actions",
        "Use agentjj bulk read/symbols/context for batch operations",
        "Run agentjj undo --to <checkpoint> to recover from mistakes",
        "Run agentjj skill to read the full documentation",
        "Run agentjj schema to see all JSON output formats",
    ];

    if json {
        let json_steps: Vec<serde_json::Value> = steps
            .iter()
            .enumerate()
            .map(|(i, (step, command, description))| {
                serde_json::json!({
                    "step": i + 1,
                    "name": step,
                    "command": command,
                    "description": description,
                })
            })
            .collect();

        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "title": "agentjj Quick Start",
                "description": "6 steps to productive version control with agentjj",
                "steps": json_steps,
                "tips": tips,
            }))?
        );
    } else {
        println!("=== agentjj Quick Start ===\n");
        for (i, (_step, command, description)) in steps.iter().enumerate() {
            println!("  {}. $ {}", i + 1, command);
            println!("     {}\n", description);
        }
        println!("Tips:");
        for tip in &tips {
            println!("  * {}", tip);
        }
    }

    Ok(())
}

/// Suggest next actions
fn cmd_suggest(json: bool) -> Result<()> {
    let mut repo = Repo::discover()?;

    let change_id = repo.current_change_id()?;
    let files = repo.changed_files(&change_id)?;
    let has_manifest = repo.has_manifest();
    let typed_change = repo.get_typed_change(&change_id).ok();

    let mut suggestions = Vec::new();

    // Based on current state, suggest actions
    if !has_manifest {
        suggestions.push(serde_json::json!({
            "action": "init",
            "command": "agentjj init",
            "reason": "No manifest found - initialize to enable full features",
            "priority": "high",
        }));
    }

    if files.is_empty() {
        suggestions.push(serde_json::json!({
            "action": "orient",
            "command": "agentjj orient",
            "reason": "No uncommitted changes - explore the codebase",
            "priority": "medium",
        }));
    } else {
        // Have changes
        if typed_change.is_none() {
            suggestions.push(serde_json::json!({
                "action": "set_change",
                "command": format!("agentjj change set -i 'describe your change' -t behavioral"),
                "reason": "Add typed change metadata for better tracking",
                "priority": "high",
            }));
        }

        suggestions.push(serde_json::json!({
            "action": "validate",
            "command": "agentjj validate",
            "reason": "Check if changes are ready to push",
            "priority": "high",
        }));

        suggestions.push(serde_json::json!({
            "action": "checkpoint",
            "command": "agentjj checkpoint work-in-progress",
            "reason": "Save a restore point before continuing",
            "priority": "medium",
        }));

        suggestions.push(serde_json::json!({
            "action": "diff",
            "command": "agentjj diff --explain",
            "reason": "Review your changes with semantic summary",
            "priority": "medium",
        }));
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "current_state": {
                    "change_id": &change_id[..12.min(change_id.len())],
                    "files_changed": files.len(),
                    "has_manifest": has_manifest,
                    "has_typed_change": typed_change.is_some(),
                },
                "suggestions": suggestions,
            }))?
        );
    } else {
        println!("=== Suggested Actions ===\n");

        for (i, s) in suggestions.iter().enumerate() {
            let priority = s["priority"].as_str().unwrap_or("medium");
            let marker = match priority {
                "high" => "!",
                _ => "-",
            };
            println!("{}. [{}] {}", i + 1, marker, s["reason"]);
            println!("   $ {}", s["command"]);
            println!();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentjj::symbols::{Symbol, SymbolKind};
    use agentjj::SupportedLanguage;
    use regex::Regex;

    #[test]
    fn test_chrono_lite_now_returns_valid_iso8601() {
        let timestamp = chrono_lite_now();
        let iso8601_regex = Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$").unwrap();
        assert!(
            iso8601_regex.is_match(&timestamp),
            "Expected ISO 8601 format, got: {}",
            timestamp
        );
    }

    #[test]
    fn test_is_leap_year_divisible_by_400() {
        assert!(
            is_leap_year(2000),
            "2000 should be a leap year (divisible by 400)"
        );
    }

    #[test]
    fn test_is_leap_year_not_divisible_by_400_but_by_100() {
        assert!(
            !is_leap_year(1900),
            "1900 should not be a leap year (divisible by 100 but not 400)"
        );
    }

    #[test]
    fn test_is_leap_year_divisible_by_4() {
        assert!(
            is_leap_year(2024),
            "2024 should be a leap year (divisible by 4)"
        );
    }

    #[test]
    fn test_is_leap_year_not_divisible_by_4() {
        assert!(!is_leap_year(2023), "2023 should not be a leap year");
    }

    #[test]
    fn test_parse_change_type_behavioral() {
        assert!(matches!(
            parse_change_type("behavioral").unwrap(),
            ChangeType::Behavioral
        ));
        assert!(matches!(
            parse_change_type("behavior").unwrap(),
            ChangeType::Behavioral
        ));
        assert!(matches!(
            parse_change_type("BEHAVIORAL").unwrap(),
            ChangeType::Behavioral
        ));
    }

    #[test]
    fn test_parse_change_type_refactor() {
        assert!(matches!(
            parse_change_type("refactor").unwrap(),
            ChangeType::Refactor
        ));
    }

    #[test]
    fn test_parse_change_type_schema() {
        assert!(matches!(
            parse_change_type("schema").unwrap(),
            ChangeType::Schema
        ));
    }

    #[test]
    fn test_parse_change_type_docs() {
        assert!(matches!(
            parse_change_type("docs").unwrap(),
            ChangeType::Docs
        ));
        assert!(matches!(
            parse_change_type("doc").unwrap(),
            ChangeType::Docs
        ));
    }

    #[test]
    fn test_parse_change_type_deps() {
        assert!(matches!(
            parse_change_type("deps").unwrap(),
            ChangeType::Deps
        ));
        assert!(matches!(
            parse_change_type("dependency").unwrap(),
            ChangeType::Deps
        ));
        assert!(matches!(
            parse_change_type("dependencies").unwrap(),
            ChangeType::Deps
        ));
    }

    #[test]
    fn test_parse_change_type_config() {
        assert!(matches!(
            parse_change_type("config").unwrap(),
            ChangeType::Config
        ));
        assert!(matches!(
            parse_change_type("configuration").unwrap(),
            ChangeType::Config
        ));
    }

    #[test]
    fn test_parse_change_type_test() {
        assert!(matches!(
            parse_change_type("test").unwrap(),
            ChangeType::Test
        ));
        assert!(matches!(
            parse_change_type("tests").unwrap(),
            ChangeType::Test
        ));
    }

    #[test]
    fn test_parse_change_type_invalid() {
        assert!(parse_change_type("invalid").is_err());
        assert!(parse_change_type("unknown").is_err());
        assert!(parse_change_type("").is_err());
    }

    #[test]
    fn test_parse_category_feature() {
        assert!(matches!(
            parse_category("feature").unwrap(),
            ChangeCategory::Feature
        ));
        assert!(matches!(
            parse_category("feat").unwrap(),
            ChangeCategory::Feature
        ));
    }

    #[test]
    fn test_parse_category_fix() {
        assert!(matches!(
            parse_category("fix").unwrap(),
            ChangeCategory::Fix
        ));
        assert!(matches!(
            parse_category("bugfix").unwrap(),
            ChangeCategory::Fix
        ));
    }

    #[test]
    fn test_parse_category_perf() {
        assert!(matches!(
            parse_category("perf").unwrap(),
            ChangeCategory::Perf
        ));
        assert!(matches!(
            parse_category("performance").unwrap(),
            ChangeCategory::Perf
        ));
    }

    #[test]
    fn test_parse_category_security() {
        assert!(matches!(
            parse_category("security").unwrap(),
            ChangeCategory::Security
        ));
        assert!(matches!(
            parse_category("sec").unwrap(),
            ChangeCategory::Security
        ));
    }

    #[test]
    fn test_parse_category_breaking() {
        assert!(matches!(
            parse_category("breaking").unwrap(),
            ChangeCategory::Breaking
        ));
    }

    #[test]
    fn test_parse_category_deprecation() {
        assert!(matches!(
            parse_category("deprecation").unwrap(),
            ChangeCategory::Deprecation
        ));
        assert!(matches!(
            parse_category("deprecate").unwrap(),
            ChangeCategory::Deprecation
        ));
    }

    #[test]
    fn test_parse_category_chore() {
        assert!(matches!(
            parse_category("chore").unwrap(),
            ChangeCategory::Chore
        ));
    }

    #[test]
    fn test_parse_category_invalid() {
        assert!(parse_category("invalid").is_err());
        assert!(parse_category("unknown").is_err());
        assert!(parse_category("").is_err());
    }

    fn make_symbol(name: &str, signature: Option<&str>) -> Symbol {
        Symbol {
            name: name.to_string(),
            kind: SymbolKind::Function,
            signature: signature.map(|s| s.to_string()),
            docstring: None,
            start_line: 1,
            end_line: 10,
            children: vec![],
        }
    }

    #[test]
    fn test_is_public_symbol_rust_pub() {
        let symbol = make_symbol("foo", Some("pub fn foo()"));
        assert!(is_public_symbol(&symbol, SupportedLanguage::Rust));
    }

    #[test]
    fn test_is_public_symbol_rust_private() {
        let symbol = make_symbol("bar", Some("fn bar()"));
        assert!(!is_public_symbol(&symbol, SupportedLanguage::Rust));
    }

    #[test]
    fn test_is_public_symbol_rust_no_signature() {
        let symbol = make_symbol("baz", None);
        assert!(!is_public_symbol(&symbol, SupportedLanguage::Rust));
    }

    #[test]
    fn test_is_public_symbol_python_public() {
        let symbol = make_symbol("my_func", Some("def my_func():"));
        assert!(is_public_symbol(&symbol, SupportedLanguage::Python));
    }

    #[test]
    fn test_is_public_symbol_python_private() {
        let symbol = make_symbol("_private", Some("def _private():"));
        assert!(!is_public_symbol(&symbol, SupportedLanguage::Python));
    }

    #[test]
    fn test_is_public_symbol_python_dunder() {
        let symbol = make_symbol("__init__", Some("def __init__(self):"));
        assert!(!is_public_symbol(&symbol, SupportedLanguage::Python));
    }

    #[test]
    fn test_is_public_symbol_js_export() {
        let symbol = make_symbol("myFunc", Some("export function myFunc()"));
        assert!(is_public_symbol(&symbol, SupportedLanguage::JavaScript));
    }

    #[test]
    fn test_is_public_symbol_js_no_export() {
        let symbol = make_symbol("myFunc", Some("function myFunc()"));
        assert!(!is_public_symbol(&symbol, SupportedLanguage::JavaScript));
    }

    #[test]
    fn test_is_public_symbol_ts_export() {
        let symbol = make_symbol("myFunc", Some("export function myFunc(): void"));
        assert!(is_public_symbol(&symbol, SupportedLanguage::TypeScript));
    }

    #[test]
    fn test_is_public_symbol_ts_no_signature_defaults_to_public() {
        let symbol = make_symbol("myFunc", None);
        assert!(is_public_symbol(&symbol, SupportedLanguage::TypeScript));
    }
}
