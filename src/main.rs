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
        #[arg(short, long)]
        change_id: Option<String>,

        /// Intent description
        #[arg(short, long)]
        intent: String,

        /// Change type
        #[arg(short = 't', long)]
        r#type: String,

        /// Category
        #[arg(short, long)]
        category: Option<String>,

        /// Mark as breaking
        #[arg(long)]
        breaking: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

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
            pr,
            title,
            body,
            target,
        } => cmd_push(branch, pr, title, body, target, cli.json),
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

    if json {
        println!(
            r#"{{"status": "created", "name": "{}", "path": ".agent/manifest.toml"}}"#,
            repo_name
        );
    } else {
        println!("Initialized agentjj for '{}'", repo_name);
        println!("Created .agent/manifest.toml");
    }

    Ok(())
}

fn cmd_status(json: bool) -> Result<()> {
    let repo = Repo::discover()?;

    let change_id = repo.current_change_id().unwrap_or_else(|_| "unknown".into());
    let operation_id = repo.current_operation_id().unwrap_or_else(|_| "unknown".into());
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
        println!("Operation: {}...", &operation_id[..16.min(operation_id.len())]);
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
    let repo = Repo::discover()?;

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
                // All changes - we need to collect differently
                Vec::new() // TODO: add all() method to ChangeIndex
            };

            if json {
                println!("{}", serde_json::to_string_pretty(&changes)?);
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
            let cid = change_id.unwrap_or_else(|| "@".to_string()); // TODO: resolve @ to actual ID
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
            _ => {
                println!("{:?}", result);
            }
        }
    }

    Ok(())
}

fn cmd_read(path: String, at: Option<String>, json: bool) -> Result<()> {
    let repo = Repo::discover()?;
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
        let repo = Repo::discover()?;
        repo.read_file(file_path, None)?
    };

    if let Some(name) = symbol_name {
        // Find specific symbol
        let symbol = agentjj::symbols::find_symbol(&content, lang, name)?;

        match symbol {
            Some(s) => {
                if json {
                    if signature_only {
                        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                            "name": s.name,
                            "signature": s.signature,
                        }))?);
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
                println!("{:>4} {:10} {}", s.start_line, format!("{:?}", s.kind).to_lowercase(), truncated);
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

fn cmd_context(path: String, json: bool) -> Result<()> {
    // Parse path: "file.py::symbol_name"
    let (file_path, symbol_name) = if let Some(idx) = path.find("::") {
        (&path[..idx], &path[idx + 2..])
    } else {
        anyhow::bail!("Symbol path must be file.py::symbol_name");
    };

    let file_path_obj = std::path::Path::new(file_path);

    // Detect language
    let lang = agentjj::SupportedLanguage::from_path(file_path_obj)
        .ok_or_else(|| anyhow::anyhow!("Unsupported file type: {}", file_path))?;

    // Read file content
    let content = if file_path_obj.is_absolute() {
        std::fs::read_to_string(file_path)?
    } else {
        let repo = Repo::discover()?;
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
                println!(r#"{{"error": "symbol not found", "name": "{}"}}"#, symbol_name);
            } else {
                println!("Symbol '{}' not found in {}", symbol_name, file_path);
            }
            std::process::exit(1);
        }
    }

    Ok(())
}

fn cmd_push(
    branch: Option<String>,
    create_pr: bool,
    title: Option<String>,
    body: Option<String>,
    target: String,
    json: bool,
) -> Result<()> {
    let repo = Repo::discover()?;

    // Determine branch name
    let branch_name = branch.unwrap_or_else(|| {
        // Default to current bookmark or generate from change ID
        repo.current_change_id()
            .map(|id| format!("agent/{}", &id[..8.min(id.len())]))
            .unwrap_or_else(|_| "agent/push".to_string())
    });

    // Create bookmark pointing to current change
    let bookmark_output = std::process::Command::new("jj")
        .current_dir(repo.root())
        .args(["bookmark", "set", &branch_name])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run jj: {}", e))?;

    if !bookmark_output.status.success() {
        let stderr = String::from_utf8_lossy(&bookmark_output.stderr);
        anyhow::bail!("Failed to create bookmark: {}", stderr);
    }

    // Push to remote
    let push_output = std::process::Command::new("jj")
        .current_dir(repo.root())
        .args(["git", "push", "--bookmark", &branch_name])
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
            let pr_url = String::from_utf8_lossy(&pr_output.stdout).trim().to_string();
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
