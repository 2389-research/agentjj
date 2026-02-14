// ABOUTME: Repository operations using jj-lib directly
// ABOUTME: Provides high-level operations for agent workflows without requiring jj CLI

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use jj_lib::backend::CommitId;
use jj_lib::config::{ConfigLayer, ConfigSource, StackedConfig};
use jj_lib::gitignore::GitIgnoreFile;
use jj_lib::matchers::{EverythingMatcher, NothingMatcher};
use jj_lib::merged_tree::MergedTreeBuilder;
use jj_lib::object_id::ObjectId;
use jj_lib::repo::{ReadonlyRepo, Repo as JjRepo, StoreFactories};
use jj_lib::repo_path::RepoPath;
use jj_lib::settings::UserSettings;
use jj_lib::working_copy::SnapshotOptions;
use jj_lib::workspace::{default_working_copy_factories, WorkingCopyFactories, Workspace};
use pollster::FutureExt as _;

use crate::change::{ChangeCategory, ChangeType, InvariantStatus, InvariantsResult, TypedChange};
use crate::error::{ConflictDetail, Error, Result};
use crate::intent::{ChangeSpec, FileOperation, Intent, IntentResult};
use crate::manifest::{InvariantTrigger, Manifest};

/// A repository handle for agent operations
pub struct Repo {
    /// Path to the repository root
    root: PathBuf,
    /// Cached workspace (loaded lazily)
    workspace: Option<Workspace>,
    /// Cached manifest (loaded lazily)
    manifest: Option<Manifest>,
}

/// Structured log entry for graph commands and other operations.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub change_id: String,
    pub commit_id: String,
    pub description: String,
    pub parent_change_ids: Vec<String>,
    pub is_working_copy: bool,
    pub timestamp: Option<String>,
    pub author: Option<String>,
    pub full_commit_id: String,
}

/// Operation info for undo and operation history commands.
#[derive(Debug, Clone)]
pub struct OperationInfo {
    pub id: String,
    pub description: String,
}

/// Options for commit_working_copy
pub struct CommitOptions {
    pub message: String,
    pub no_new: bool,
    pub run_invariants: bool,
    pub change_type: ChangeType,
    pub category: Option<ChangeCategory>,
    pub breaking: bool,
    /// When set, only changes to these paths are included in the commit.
    /// Unlisted changes remain in the working copy.
    pub paths: Option<Vec<String>>,
}

/// Result of a successful commit via jj-lib
pub struct CommitResult {
    pub change_id: String,
    pub commit_id: String,
    pub operation_id: String,
    pub files_changed: Vec<String>,
    pub invariants: HashMap<String, InvariantStatus>,
}

/// Load base gitignore rules for working copy snapshots. Mirrors what the
/// jj CLI does: reads the global gitignore and .git/info/exclude so that
/// the snapshot respects all ignore layers (global, repo-level, per-dir).
fn load_base_ignores(root: &Path) -> Arc<GitIgnoreFile> {
    let mut ignores = GitIgnoreFile::empty();

    // 1. Global gitignore: check git config core.excludesFile, then XDG default
    let global_path = Command::new("git")
        .current_dir(root)
        .args(["config", "--get", "core.excludesFile"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let p = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if p.is_empty() {
                    None
                } else {
                    // Expand ~ in path
                    Some(if let Some(stripped) = p.strip_prefix("~/") {
                        dirs::home_dir().unwrap_or_default().join(stripped)
                    } else {
                        PathBuf::from(p)
                    })
                }
            } else {
                None
            }
        })
        .or_else(|| dirs::config_dir().map(|d| d.join("git").join("ignore")));

    if let Some(path) = global_path {
        if let Ok(chained) = ignores.chain_with_file("", path) {
            ignores = chained;
        }
    }

    // 2. .git/info/exclude
    let exclude_path = root.join(".git").join("info").join("exclude");
    if let Ok(chained) = ignores.chain_with_file("", exclude_path) {
        ignores = chained;
    }

    ignores
}

/// Creates minimal UserSettings for agentjj operations.
/// These settings are used when we don't need user's full config.
fn create_minimal_settings() -> std::result::Result<UserSettings, Error> {
    let mut config = StackedConfig::with_defaults();

    // Add minimal required settings
    let layer = ConfigLayer::parse(
        ConfigSource::CommandArg,
        r#"
[user]
name = "agentjj"
email = "agentjj@localhost"

[operation]
hostname = "agentjj"
username = "agentjj"

[signing]
behavior = "drop"
"#,
    )
    .map_err(|e| Error::Repository {
        message: format!("failed to create config: {}", e),
    })?;

    config.add_layer(layer);

    UserSettings::from_config(config).map_err(|e| Error::Repository {
        message: format!("failed to create settings: {}", e),
    })
}

/// Get the default store factories for loading repositories
fn get_store_factories() -> StoreFactories {
    StoreFactories::default()
}

/// Get the default working copy factories
fn get_working_copy_factories() -> WorkingCopyFactories {
    default_working_copy_factories()
}

impl Repo {
    /// Open a repository at the given path
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let root = path.as_ref().to_path_buf();
        Ok(Self {
            root,
            workspace: None,
            manifest: None,
        })
    }

    /// Discover and open a repository from the current directory or ancestors.
    /// If a git repo is found without jj, automatically colocates jj with it.
    /// Resolves symlinks to ensure consistent paths (jj's working copy tracking
    /// uses filesystem paths, so symlinked working directories fail silently).
    pub fn discover() -> Result<Self> {
        let cwd = std::env::current_dir()?;
        // Resolve symlinks so jj's working copy tracking uses the canonical path
        let canonical_cwd = std::fs::canonicalize(&cwd).unwrap_or(cwd);
        let mut current = canonical_cwd.as_path();

        // Track if we find a git repo without jj
        let mut found_git_without_jj: Option<PathBuf> = None;

        loop {
            let has_jj = current.join(".jj").exists();
            let has_git = current.join(".git").exists();

            if has_jj {
                return Self::open(current);
            }

            // Record git repo without jj for auto-colocate
            if has_git && found_git_without_jj.is_none() {
                found_git_without_jj = Some(current.to_path_buf());
            }

            match current.parent() {
                Some(parent) => current = parent,
                None => {
                    // No jj repo found - auto-colocate if git repo exists
                    if let Some(git_path) = found_git_without_jj {
                        return Self::init_colocated_git(&git_path);
                    }
                    return Err(Error::Repository {
                        message: "No git or jj repository found (or any parent)".into(),
                    });
                }
            }
        }
    }

    /// Initialize jj colocated with an existing git repository.
    /// This is called automatically when discover() finds a git repo without jj.
    fn init_colocated_git(git_repo_path: &Path) -> Result<Self> {
        let settings = create_minimal_settings()?;

        // Use init_external_git for existing git repos - pass the .git path
        let git_dir = git_repo_path.join(".git");
        let (_workspace, _repo) = Workspace::init_external_git(&settings, git_repo_path, &git_dir)
            .map_err(|e| Error::Repository {
                message: format!("Failed to initialize jj with git repo: {}", e),
            })?;

        // Ensure .jj is in .gitignore so it doesn't pollute git status
        Self::ensure_jj_gitignored(git_repo_path);

        // Now open the newly created repo
        Self::open(git_repo_path)
    }

    /// Add .jj/ to .gitignore if not already present.
    fn ensure_jj_gitignored(repo_path: &Path) {
        let gitignore_path = repo_path.join(".gitignore");
        let content = std::fs::read_to_string(&gitignore_path).unwrap_or_default();

        // Check if .jj is already ignored (various forms)
        let already_ignored = content.lines().any(|line| {
            let trimmed = line.trim();
            trimmed == ".jj" || trimmed == ".jj/" || trimmed == "/.jj" || trimmed == "/.jj/"
        });

        if !already_ignored {
            let new_content = if content.is_empty() || content.ends_with('\n') {
                format!("{}.jj/\n", content)
            } else {
                format!("{}\n.jj/\n", content)
            };
            if let Err(e) = std::fs::write(&gitignore_path, new_content) {
                eprintln!("warning: failed to add .jj to .gitignore: {}", e);
            }
        }
    }

    /// Get the repository root path
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Load the workspace lazily
    fn load_workspace(&mut self) -> Result<&Workspace> {
        if self.workspace.is_none() {
            let settings = create_minimal_settings()?;
            let store_factories = get_store_factories();
            let wc_factories = get_working_copy_factories();

            let workspace = Workspace::load(&settings, &self.root, &store_factories, &wc_factories)
                .map_err(|e| Error::Repository {
                    message: format!("failed to load workspace: {}", e),
                })?;

            self.workspace = Some(workspace);
        }
        Ok(self.workspace.as_ref().unwrap())
    }

    /// Load the repository at HEAD
    fn load_repo_at_head(&mut self) -> Result<Arc<ReadonlyRepo>> {
        let workspace = self.load_workspace()?;
        workspace
            .repo_loader()
            .load_at_head()
            .map_err(|e| Error::Repository {
                message: format!("failed to load repository: {}", e),
            })
    }

    /// Get or load the manifest
    pub fn manifest(&mut self) -> Result<&Manifest> {
        if self.manifest.is_none() {
            self.manifest = Some(Manifest::load_from_repo(&self.root)?);
        }
        Ok(self.manifest.as_ref().unwrap())
    }

    /// Check if a manifest exists
    pub fn has_manifest(&self) -> bool {
        self.root.join(Manifest::DEFAULT_PATH).exists()
    }

    /// Get the current change ID (@ in jj)
    pub fn current_change_id(&mut self) -> Result<String> {
        let repo = self.load_repo_at_head()?;

        // Get the workspace's working copy commit
        let workspace = self.workspace.as_ref().unwrap();
        let wc_commit_id = repo
            .view()
            .get_wc_commit_id(workspace.workspace_name())
            .ok_or_else(|| Error::Repository {
                message: "no working copy commit found".into(),
            })?;

        let commit = repo
            .store()
            .get_commit(wc_commit_id)
            .map_err(|e| Error::Repository {
                message: format!("failed to get commit: {}", e),
            })?;

        Ok(commit.change_id().hex())
    }

    /// Get the current commit ID
    pub fn current_commit_id(&mut self) -> Result<String> {
        let repo = self.load_repo_at_head()?;

        let workspace = self.workspace.as_ref().unwrap();
        let wc_commit_id = repo
            .view()
            .get_wc_commit_id(workspace.workspace_name())
            .ok_or_else(|| Error::Repository {
                message: "no working copy commit found".into(),
            })?;

        Ok(wc_commit_id.hex())
    }

    /// Get current operation ID
    pub fn current_operation_id(&mut self) -> Result<String> {
        let repo = self.load_repo_at_head()?;
        Ok(repo.op_id().hex())
    }

    /// Read file content at a specific change or branch
    pub fn read_file(&mut self, path: &str, at: Option<&str>) -> Result<String> {
        // If no revision specified, just read from working copy on disk
        // This handles both tracked and untracked files
        if at.is_none() {
            let full_path = self.root.join(path);
            return std::fs::read_to_string(&full_path).map_err(|e| Error::Repository {
                message: format!("failed to read file '{}': {}", path, e),
            });
        }

        // For specific revisions, we need to look up in the repository
        let repo = self.load_repo_at_head()?;
        let workspace = self.workspace.as_ref().unwrap();
        let rev = at.unwrap();

        // Get the commit to read from
        let commit_id = if rev == "@" {
            repo.view()
                .get_wc_commit_id(workspace.workspace_name())
                .cloned()
                .ok_or_else(|| Error::Repository {
                    message: "no working copy commit found".into(),
                })?
        } else {
            // Try to parse as commit ID hex prefix
            CommitId::try_from_hex(rev).ok_or_else(|| Error::Repository {
                message: format!(
                    "cannot resolve revision '{}' - only @ and commit IDs are supported via jj-lib",
                    rev
                ),
            })?
        };

        let commit = repo
            .store()
            .get_commit(&commit_id)
            .map_err(|e| Error::Repository {
                message: format!("failed to get commit: {}", e),
            })?;

        // Get the tree and read the file
        let tree = commit.tree();
        let repo_path =
            jj_lib::repo_path::RepoPathBuf::from_internal_string(path).map_err(|e| {
                Error::Repository {
                    message: format!("invalid path '{}': {}", path, e),
                }
            })?;

        let value = tree.path_value(&repo_path).map_err(|e| Error::Repository {
            message: format!("failed to read tree: {}", e),
        })?;

        // Check if file exists and is a normal file
        if value.is_absent() {
            return Err(Error::Repository {
                message: format!("file '{}' not found at revision '{}'", path, rev),
            });
        }

        // Get file content
        let content = value
            .into_resolved()
            .map_err(|_| Error::Repository {
                message: format!("file '{}' has conflicts at revision '{}'", path, rev),
            })?
            .ok_or_else(|| Error::Repository {
                message: format!("file '{}' not found at revision '{}'", path, rev),
            })?;

        match content {
            jj_lib::backend::TreeValue::File { .. } => {
                // For specific revisions, we still read from working copy
                // (In a full implementation, we'd read from the store)
                let full_path = self.root.join(path);
                std::fs::read_to_string(&full_path).map_err(|e| Error::Repository {
                    message: format!("failed to read file: {}", e),
                })
            }
            jj_lib::backend::TreeValue::Symlink(_target_id) => {
                // Read symlink target from working copy
                let full_path = self.root.join(path);
                std::fs::read_link(&full_path)
                    .map(|p| p.to_string_lossy().to_string())
                    .map_err(|e| Error::Repository {
                        message: format!("failed to read symlink: {}", e),
                    })
            }
            _ => Err(Error::Repository {
                message: format!("'{}' is not a regular file", path),
            }),
        }
    }

    /// List files changed in a specific change
    pub fn changed_files(&mut self, change_id: &str) -> Result<Vec<String>> {
        let repo = self.load_repo_at_head()?;

        // Try to find commit by change ID
        let change_id_obj =
            jj_lib::backend::ChangeId::try_from_hex(change_id).ok_or_else(|| {
                Error::Repository {
                    message: format!("invalid change ID: {}", change_id),
                }
            })?;

        let targets = repo
            .resolve_change_id(&change_id_obj)
            .map_err(|e| Error::Repository {
                message: format!("failed to resolve change ID: {}", e),
            })?
            .ok_or_else(|| Error::Repository {
                message: format!("change '{}' not found", change_id),
            })?;

        // Get the first visible commit for this change
        let (_, commit_id) =
            targets
                .visible_with_offsets()
                .next()
                .ok_or_else(|| Error::Repository {
                    message: format!("no visible commits for change '{}'", change_id),
                })?;

        let commit = repo
            .store()
            .get_commit(commit_id)
            .map_err(|e| Error::Repository {
                message: format!("failed to get commit: {}", e),
            })?;

        // Get parent tree for diff
        let parent_tree = commit.parent_tree(&*repo).map_err(|e| Error::Repository {
            message: format!("failed to get parent tree: {}", e),
        })?;

        let tree = commit.tree();

        // Diff the trees using synchronous iterator
        let mut files = Vec::new();
        let diff_iter = jj_lib::merged_tree::TreeDiffIterator::new(
            &parent_tree,
            &tree,
            &jj_lib::matchers::EverythingMatcher,
        );
        for diff_entry in diff_iter {
            files.push(diff_entry.path.as_internal_file_string().to_string());
        }

        Ok(files)
    }

    /// Check if a branch/bookmark exists and get its change ID
    pub fn branch_change_id(&mut self, branch: &str) -> Result<Option<String>> {
        let repo = self.load_repo_at_head()?;

        let ref_name: &jj_lib::ref_name::RefName = branch.as_ref();
        let target = repo.view().get_local_bookmark(ref_name);

        if target.is_absent() {
            return Ok(None);
        }

        // Get the first commit from the target
        let commit_id = target.added_ids().next().ok_or_else(|| Error::Repository {
            message: format!("bookmark '{}' has no commits", branch),
        })?;

        let commit = repo
            .store()
            .get_commit(commit_id)
            .map_err(|e| Error::Repository {
                message: format!("failed to get commit: {}", e),
            })?;

        Ok(Some(commit.change_id().hex()))
    }

    /// Check if a change has conflicts
    pub fn has_conflicts(&mut self, change_id: &str) -> Result<bool> {
        let repo = self.load_repo_at_head()?;

        let change_id_obj =
            jj_lib::backend::ChangeId::try_from_hex(change_id).ok_or_else(|| {
                Error::Repository {
                    message: format!("invalid change ID: {}", change_id),
                }
            })?;

        let targets = repo
            .resolve_change_id(&change_id_obj)
            .map_err(|e| Error::Repository {
                message: format!("failed to resolve change ID: {}", e),
            })?;

        if let Some(targets) = targets {
            for (_, commit_id) in targets.visible_with_offsets() {
                let commit = repo
                    .store()
                    .get_commit(commit_id)
                    .map_err(|e| Error::Repository {
                        message: format!("failed to get commit: {}", e),
                    })?;
                if commit.has_conflict() {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Get conflict details for a change
    pub fn get_conflicts(&mut self, change_id: &str) -> Result<Vec<ConflictDetail>> {
        let repo = self.load_repo_at_head()?;

        let change_id_obj =
            jj_lib::backend::ChangeId::try_from_hex(change_id).ok_or_else(|| {
                Error::Repository {
                    message: format!("invalid change ID: {}", change_id),
                }
            })?;

        let targets = repo
            .resolve_change_id(&change_id_obj)
            .map_err(|e| Error::Repository {
                message: format!("failed to resolve change ID: {}", e),
            })?
            .ok_or_else(|| Error::Repository {
                message: format!("change '{}' not found", change_id),
            })?;

        let mut conflicts = Vec::new();

        for (_, commit_id) in targets.visible_with_offsets() {
            let commit = repo
                .store()
                .get_commit(commit_id)
                .map_err(|e| Error::Repository {
                    message: format!("failed to get commit: {}", e),
                })?;

            if commit.has_conflict() {
                let tree = commit.tree();
                // Iterate through conflicted paths
                for (path, _value) in tree.entries() {
                    conflicts.push(ConflictDetail {
                        file: path.as_internal_file_string().to_string(),
                        ours: String::new(),   // TODO: extract actual content
                        theirs: String::new(), // TODO: extract actual content
                        base: None,
                    });
                }
            }
        }

        Ok(conflicts)
    }

    /// Apply an intent to the repository
    pub fn apply(&mut self, intent: Intent) -> Result<IntentResult> {
        // 1. Check preconditions
        if let Err(e) = self.check_preconditions(&intent) {
            return Ok(e);
        }

        // 2. Check permissions if manifest exists
        if self.has_manifest() {
            if let Err(e) = self.check_permissions(&intent) {
                return Ok(e);
            }
        }

        // 3. Create a new change using jj-lib transaction
        let (change_id, operation_id) = self.create_new_change(&intent.description)?;

        // 4. Apply changes
        let files_changed = match self.apply_changes(&intent.changes) {
            Ok(files) => files,
            Err(e) => {
                // Rollback on error - undo the last operation
                let _ = self.undo_operation();
                return Err(e);
            }
        };

        // 5. Check for conflicts
        if self.has_conflicts(&change_id)? {
            let conflicts = self.get_conflicts(&change_id)?;
            let prev_op = self.get_previous_op_id()?;
            return Ok(IntentResult::Conflict {
                change_id,
                operation_id: operation_id.clone(),
                conflicts,
                rollback_command: format!("jj op restore {}", prev_op),
            });
        }

        // 6. Check for paths requiring human review
        if self.has_manifest() {
            let manifest = self.manifest()?.clone();
            let review_paths: Vec<String> = files_changed
                .iter()
                .filter(|f| manifest.requires_human_review(f))
                .cloned()
                .collect();

            if !review_paths.is_empty() {
                return Ok(IntentResult::RequiresReview {
                    change_id,
                    paths: review_paths,
                    message: "These paths require human review before merge".to_string(),
                });
            }
        }

        // 7. Run invariants
        let invariants = if intent.run_invariants && self.has_manifest() {
            match self.run_invariants(InvariantTrigger::PreCommit) {
                Ok(results) => results,
                Err((name, cmd, code, stdout, stderr)) => {
                    let prev_op = self.get_previous_op_id()?;
                    return Ok(IntentResult::InvariantFailed {
                        invariant: name,
                        command: cmd,
                        exit_code: code,
                        stdout,
                        stderr,
                        change_id,
                        rollback_command: format!("jj op restore {}", prev_op),
                    });
                }
            }
        } else {
            HashMap::new()
        };

        // 8. Save typed change metadata
        let typed_change =
            TypedChange::new(change_id.clone(), intent.change_type, &intent.description)
                .with_files(files_changed.clone());
        let typed_change = if intent.breaking {
            typed_change.breaking()
        } else {
            typed_change
        };
        let mut typed_change = typed_change;
        typed_change.invariants = InvariantsResult {
            checked: invariants.keys().cloned().collect(),
            status: if invariants.values().all(|s| *s == InvariantStatus::Passed) {
                InvariantStatus::Passed
            } else {
                InvariantStatus::Failed
            },
            details: invariants.clone(),
        };
        self.save_typed_change(&typed_change)?;

        Ok(IntentResult::Success {
            change_id,
            operation_id,
            files_changed,
            invariants,
            pr_url: None,
        })
    }

    /// Create a new change using jj-lib
    fn create_new_change(&mut self, description: &str) -> Result<(String, String)> {
        let settings = create_minimal_settings()?;
        let store_factories = get_store_factories();
        let wc_factories = get_working_copy_factories();

        // Reload workspace to get fresh state
        let workspace = Workspace::load(&settings, &self.root, &store_factories, &wc_factories)
            .map_err(|e| Error::Repository {
                message: format!("failed to load workspace: {}", e),
            })?;

        let repo = workspace
            .repo_loader()
            .load_at_head()
            .map_err(|e| Error::Repository {
                message: format!("failed to load repository: {}", e),
            })?;

        // Get current working copy commit
        let wc_commit_id = repo
            .view()
            .get_wc_commit_id(workspace.workspace_name())
            .cloned()
            .ok_or_else(|| Error::Repository {
                message: "no working copy commit found".into(),
            })?;

        let parent_commit =
            repo.store()
                .get_commit(&wc_commit_id)
                .map_err(|e| Error::Repository {
                    message: format!("failed to get commit: {}", e),
                })?;

        // Start a transaction
        let mut tx = repo.start_transaction();

        // Create new commit with the same tree as parent (empty change)
        let new_commit = tx
            .repo_mut()
            .new_commit(vec![wc_commit_id], parent_commit.tree())
            .set_description(description)
            .write()
            .map_err(|e| Error::Repository {
                message: format!("failed to create commit: {}", e),
            })?;

        // Update working copy to point to new commit
        tx.repo_mut()
            .set_wc_commit(
                workspace.workspace_name().to_owned(),
                new_commit.id().clone(),
            )
            .map_err(|e| Error::Repository {
                message: format!("failed to set working copy: {}", e),
            })?;

        // Commit the transaction
        let new_repo = tx.commit("new change").map_err(|e| Error::Repository {
            message: format!("failed to commit transaction: {}", e),
        })?;

        let change_id = new_commit.change_id().hex();
        let operation_id = new_repo.op_id().hex();

        // Update our cached workspace
        self.workspace = None; // Force reload on next access

        Ok((change_id, operation_id))
    }

    /// Undo the last operation
    fn undo_operation(&mut self) -> Result<()> {
        let settings = create_minimal_settings()?;
        let store_factories = get_store_factories();
        let wc_factories = get_working_copy_factories();

        let workspace = Workspace::load(&settings, &self.root, &store_factories, &wc_factories)
            .map_err(|e| Error::Repository {
                message: format!("failed to load workspace: {}", e),
            })?;

        let repo = workspace
            .repo_loader()
            .load_at_head()
            .map_err(|e| Error::Repository {
                message: format!("failed to load repository: {}", e),
            })?;

        // Get parent operation
        let current_op = repo.operation();
        let parent_ops: Vec<_> = current_op
            .parents()
            .collect::<std::result::Result<_, _>>()
            .map_err(|e| Error::Repository {
                message: format!("failed to get parent operations: {}", e),
            })?;

        if parent_ops.is_empty() {
            return Err(Error::Repository {
                message: "no parent operation to undo".into(),
            });
        }

        // Load repo at parent operation
        let _parent_repo = workspace
            .repo_loader()
            .load_at(&parent_ops[0])
            .map_err(|e| Error::Repository {
                message: format!("failed to load parent operation: {}", e),
            })?;

        // Force workspace reload
        self.workspace = None;

        Ok(())
    }

    /// Check preconditions for an intent
    #[allow(clippy::result_large_err)]
    fn check_preconditions(&mut self, intent: &Intent) -> std::result::Result<(), IntentResult> {
        let preconds = &intent.preconditions;

        // Check operation ID
        if let Some(expected_op) = &preconds.operation_id {
            let actual = self.current_operation_id().unwrap_or_default();
            if &actual != expected_op {
                return Err(IntentResult::PreconditionFailed {
                    reason: "operation ID mismatch".to_string(),
                    expected: expected_op.clone(),
                    actual,
                });
            }
        }

        // Check branch positions
        for (branch, expected_change) in &preconds.branch_at {
            let actual = self.branch_change_id(branch).ok().flatten();
            match actual {
                Some(actual_id) if &actual_id != expected_change => {
                    return Err(IntentResult::PreconditionFailed {
                        reason: format!("branch '{}' has moved", branch),
                        expected: expected_change.clone(),
                        actual: actual_id,
                    });
                }
                None => {
                    return Err(IntentResult::PreconditionFailed {
                        reason: format!("branch '{}' not found", branch),
                        expected: expected_change.clone(),
                        actual: "not found".to_string(),
                    });
                }
                _ => {}
            }
        }

        // Check file existence
        for path in &preconds.files_exist {
            let full_path = self.root.join(path);
            if !full_path.exists() {
                return Err(IntentResult::PreconditionFailed {
                    reason: format!("file '{}' does not exist", path),
                    expected: "exists".to_string(),
                    actual: "not found".to_string(),
                });
            }
        }

        for path in &preconds.files_absent {
            let full_path = self.root.join(path);
            if full_path.exists() {
                return Err(IntentResult::PreconditionFailed {
                    reason: format!("file '{}' should not exist", path),
                    expected: "absent".to_string(),
                    actual: "exists".to_string(),
                });
            }
        }

        // Check file hashes
        for (path, expected_hash) in &preconds.file_hashes {
            let full_path = self.root.join(path);
            if !full_path.exists() {
                return Err(IntentResult::PreconditionFailed {
                    reason: format!("file '{}' not found for hash check", path),
                    expected: expected_hash.clone(),
                    actual: "file not found".to_string(),
                });
            }

            match std::fs::read(&full_path) {
                Ok(content) => {
                    use sha2::{Digest, Sha256};
                    let mut hasher = Sha256::new();
                    hasher.update(&content);
                    let actual_hash = hex::encode(hasher.finalize());

                    if actual_hash != expected_hash.to_lowercase() {
                        return Err(IntentResult::PreconditionFailed {
                            reason: format!("file '{}' hash mismatch", path),
                            expected: expected_hash.clone(),
                            actual: actual_hash,
                        });
                    }
                }
                Err(e) => {
                    return Err(IntentResult::PreconditionFailed {
                        reason: format!("failed to read file '{}': {}", path, e),
                        expected: expected_hash.clone(),
                        actual: "read error".to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Check permissions for an intent
    #[allow(clippy::result_large_err)]
    fn check_permissions(&mut self, intent: &Intent) -> std::result::Result<(), IntentResult> {
        let manifest = match self.manifest() {
            Ok(m) => m.clone(),
            Err(_) => return Ok(()), // No manifest means no permission restrictions
        };

        // Get files that will be changed
        let files = match &intent.changes {
            ChangeSpec::Files { operations } => operations
                .iter()
                .map(|op| match op {
                    FileOperation::Create { path, .. } => path.clone(),
                    FileOperation::Replace { path, .. } => path.clone(),
                    FileOperation::Delete { path } => path.clone(),
                    FileOperation::Rename { from, to } => format!("{} -> {}", from, to),
                })
                .collect::<Vec<_>>(),
            _ => vec![], // Can't easily know files from a patch
        };

        for file in files {
            if !manifest.permissions.can_change(&file) {
                return Err(IntentResult::PermissionDenied {
                    action: "change".to_string(),
                    path: file,
                    rule: "deny_change or not in allow_change".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Apply changes from a ChangeSpec
    fn apply_changes(&self, changes: &ChangeSpec) -> Result<Vec<String>> {
        match changes {
            ChangeSpec::Patch { content } => {
                // Write patch to temp file and apply
                let patch_path = self.root.join(".agent/temp.patch");
                if let Some(parent) = patch_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&patch_path, content)?;

                // Apply patch using system patch command
                let output = Command::new("patch")
                    .args(["-p1", "-i", ".agent/temp.patch"])
                    .current_dir(&self.root)
                    .output()
                    .map_err(|e| Error::Repository {
                        message: format!("failed to run patch: {}", e),
                    })?;

                std::fs::remove_file(&patch_path).ok();

                if !output.status.success() {
                    return Err(Error::Repository {
                        message: format!(
                            "patch failed: {}",
                            String::from_utf8_lossy(&output.stderr)
                        ),
                    });
                }

                // Return empty list - caller should check jj status
                Ok(vec![])
            }

            ChangeSpec::PatchFile { path } => {
                let content = std::fs::read_to_string(path)?;
                self.apply_changes(&ChangeSpec::Patch { content })
            }

            ChangeSpec::Files { operations } => {
                let mut files = Vec::new();

                for op in operations {
                    match op {
                        FileOperation::Create { path, content } => {
                            let full_path = self.root.join(path);
                            if let Some(parent) = full_path.parent() {
                                std::fs::create_dir_all(parent)?;
                            }
                            std::fs::write(&full_path, content)?;
                            files.push(path.clone());
                        }
                        FileOperation::Replace { path, content } => {
                            let full_path = self.root.join(path);
                            std::fs::write(&full_path, content)?;
                            files.push(path.clone());
                        }
                        FileOperation::Delete { path } => {
                            let full_path = self.root.join(path);
                            std::fs::remove_file(&full_path)?;
                            files.push(path.clone());
                        }
                        FileOperation::Rename { from, to } => {
                            let from_path = self.root.join(from);
                            let to_path = self.root.join(to);
                            std::fs::rename(&from_path, &to_path)?;
                            files.push(from.clone());
                            files.push(to.clone());
                        }
                    }
                }

                Ok(files)
            }
        }
    }

    /// Run invariants and return results
    #[allow(clippy::type_complexity)]
    fn run_invariants(
        &mut self,
        trigger: InvariantTrigger,
    ) -> std::result::Result<HashMap<String, InvariantStatus>, (String, String, i32, String, String)>
    {
        let manifest = match self.manifest() {
            Ok(m) => m.clone(),
            Err(_) => return Ok(HashMap::new()), // No manifest means no invariants
        };
        let invariants = manifest.invariants_for(trigger);
        let mut results = HashMap::new();

        for (name, invariant) in invariants {
            let cmd = invariant.command();

            // Run the command via shell
            let output = Command::new("sh")
                .args(["-c", cmd])
                .current_dir(&self.root)
                .output();

            match output {
                Ok(out) if out.status.success() => {
                    results.insert(name.to_string(), InvariantStatus::Passed);
                }
                Ok(out) => {
                    return Err((
                        name.to_string(),
                        cmd.to_string(),
                        out.status.code().unwrap_or(-1),
                        String::from_utf8_lossy(&out.stdout).to_string(),
                        String::from_utf8_lossy(&out.stderr).to_string(),
                    ));
                }
                Err(e) => {
                    return Err((
                        name.to_string(),
                        cmd.to_string(),
                        -1,
                        String::new(),
                        e.to_string(),
                    ));
                }
            }
        }

        Ok(results)
    }

    /// Get the previous operation ID (for rollback)
    fn get_previous_op_id(&mut self) -> Result<String> {
        let repo = self.load_repo_at_head()?;

        let current_op = repo.operation();
        let parent_ops: Vec<_> = current_op
            .parents()
            .collect::<std::result::Result<_, _>>()
            .map_err(|e| Error::Repository {
                message: format!("failed to get parent operations: {}", e),
            })?;

        if parent_ops.is_empty() {
            Ok(current_op.id().hex())
        } else {
            Ok(parent_ops[0].id().hex())
        }
    }

    /// Get typed change metadata by change ID
    pub fn get_typed_change(&self, change_id: &str) -> Result<TypedChange> {
        TypedChange::load_from_repo(&self.root, change_id)
    }

    /// Save typed change metadata
    pub fn save_typed_change(&self, change: &TypedChange) -> Result<()> {
        change.save(&self.root)
    }

    /// Describe the current change
    pub fn describe(&mut self, message: &str) -> Result<()> {
        let settings = create_minimal_settings()?;
        let store_factories = get_store_factories();
        let wc_factories = get_working_copy_factories();

        let workspace = Workspace::load(&settings, &self.root, &store_factories, &wc_factories)
            .map_err(|e| Error::Repository {
                message: format!("failed to load workspace: {}", e),
            })?;

        let repo = workspace
            .repo_loader()
            .load_at_head()
            .map_err(|e| Error::Repository {
                message: format!("failed to load repository: {}", e),
            })?;

        let wc_commit_id = repo
            .view()
            .get_wc_commit_id(workspace.workspace_name())
            .cloned()
            .ok_or_else(|| Error::Repository {
                message: "no working copy commit found".into(),
            })?;

        let commit = repo
            .store()
            .get_commit(&wc_commit_id)
            .map_err(|e| Error::Repository {
                message: format!("failed to get commit: {}", e),
            })?;

        // Start transaction
        let mut tx = repo.start_transaction();

        // Rewrite commit with new description
        let new_commit = tx
            .repo_mut()
            .rewrite_commit(&commit)
            .set_description(message)
            .write()
            .map_err(|e| Error::Repository {
                message: format!("failed to rewrite commit: {}", e),
            })?;

        // Update working copy
        tx.repo_mut()
            .set_wc_commit(
                workspace.workspace_name().to_owned(),
                new_commit.id().clone(),
            )
            .map_err(|e| Error::Repository {
                message: format!("failed to set working copy: {}", e),
            })?;

        // Rebase descendants
        tx.repo_mut()
            .rebase_descendants()
            .map_err(|e| Error::Repository {
                message: format!("failed to rebase descendants: {}", e),
            })?;

        // Commit transaction
        tx.commit("describe").map_err(|e| Error::Repository {
            message: format!("failed to commit transaction: {}", e),
        })?;

        // Clear cached workspace
        self.workspace = None;

        Ok(())
    }

    /// Create a new change
    pub fn new_change(&mut self, message: Option<&str>) -> Result<String> {
        let desc = message.unwrap_or("");
        let (change_id, _) = self.create_new_change(desc)?;
        Ok(change_id)
    }

    /// Squash changes into parent
    pub fn squash(&mut self) -> Result<()> {
        let settings = create_minimal_settings()?;
        let store_factories = get_store_factories();
        let wc_factories = get_working_copy_factories();

        let workspace = Workspace::load(&settings, &self.root, &store_factories, &wc_factories)
            .map_err(|e| Error::Repository {
                message: format!("failed to load workspace: {}", e),
            })?;

        let repo = workspace
            .repo_loader()
            .load_at_head()
            .map_err(|e| Error::Repository {
                message: format!("failed to load repository: {}", e),
            })?;

        let wc_commit_id = repo
            .view()
            .get_wc_commit_id(workspace.workspace_name())
            .cloned()
            .ok_or_else(|| Error::Repository {
                message: "no working copy commit found".into(),
            })?;

        let commit = repo
            .store()
            .get_commit(&wc_commit_id)
            .map_err(|e| Error::Repository {
                message: format!("failed to get commit: {}", e),
            })?;

        // Get parent commit
        let parent_ids = commit.parent_ids();
        if parent_ids.is_empty() {
            return Err(Error::Repository {
                message: "cannot squash: no parent commit".into(),
            });
        }

        let parent = repo
            .store()
            .get_commit(&parent_ids[0])
            .map_err(|e| Error::Repository {
                message: format!("failed to get parent commit: {}", e),
            })?;

        // Start transaction
        let mut tx = repo.start_transaction();

        // Create new commit with current tree but parent's parents
        let new_description = if commit.description().is_empty() {
            parent.description().to_string()
        } else if parent.description().is_empty() {
            commit.description().to_string()
        } else {
            format!("{}\n\n{}", parent.description(), commit.description())
        };

        let new_commit = tx
            .repo_mut()
            .new_commit(parent.parent_ids().to_vec(), commit.tree())
            .set_description(&new_description)
            .write()
            .map_err(|e| Error::Repository {
                message: format!("failed to create squashed commit: {}", e),
            })?;

        // Record the rewrites
        tx.repo_mut()
            .set_rewritten_commit(commit.id().clone(), new_commit.id().clone());
        tx.repo_mut()
            .set_rewritten_commit(parent.id().clone(), new_commit.id().clone());

        // Update working copy
        tx.repo_mut()
            .set_wc_commit(
                workspace.workspace_name().to_owned(),
                new_commit.id().clone(),
            )
            .map_err(|e| Error::Repository {
                message: format!("failed to set working copy: {}", e),
            })?;

        // Rebase descendants
        tx.repo_mut()
            .rebase_descendants()
            .map_err(|e| Error::Repository {
                message: format!("failed to rebase descendants: {}", e),
            })?;

        // Commit transaction
        tx.commit("squash").map_err(|e| Error::Repository {
            message: format!("failed to commit transaction: {}", e),
        })?;

        // Clear cached workspace
        self.workspace = None;

        Ok(())
    }

    /// Resolve a jj revision spec to its commit ID hex and parent commit ID hex.
    /// Supports @, @-, and jj change ID hex prefixes.
    /// In colocated mode, jj commit IDs are git commit IDs.
    pub fn resolve_revision(&mut self, rev: &str) -> Result<(Option<String>, String)> {
        let repo = self.load_repo_at_head()?;
        let workspace = self.workspace.as_ref().unwrap();

        let commit_id = match rev {
            "@" => repo
                .view()
                .get_wc_commit_id(workspace.workspace_name())
                .cloned()
                .ok_or_else(|| Error::Repository {
                    message: "no working copy commit found".into(),
                })?,
            "@-" => {
                let wc_id = repo
                    .view()
                    .get_wc_commit_id(workspace.workspace_name())
                    .cloned()
                    .ok_or_else(|| Error::Repository {
                        message: "no working copy commit found".into(),
                    })?;
                let wc_commit = repo
                    .store()
                    .get_commit(&wc_id)
                    .map_err(|e| Error::Repository {
                        message: format!("failed to get commit: {}", e),
                    })?;
                wc_commit
                    .parent_ids()
                    .first()
                    .cloned()
                    .ok_or_else(|| Error::Repository {
                        message: "working copy has no parent".into(),
                    })?
            }
            other => {
                let change_id_obj =
                    jj_lib::backend::ChangeId::try_from_hex(other).ok_or_else(|| {
                        Error::Repository {
                            message: format!("invalid revision: {}", other),
                        }
                    })?;
                let targets = repo
                    .resolve_change_id(&change_id_obj)
                    .map_err(|e| Error::Repository {
                        message: format!("failed to resolve change ID: {}", e),
                    })?
                    .ok_or_else(|| Error::Repository {
                        message: format!("change '{}' not found", other),
                    })?;
                let (_, cid) =
                    targets
                        .visible_with_offsets()
                        .next()
                        .ok_or_else(|| Error::Repository {
                            message: format!("no visible commits for '{}'", other),
                        })?;
                cid.clone()
            }
        };

        let commit = repo
            .store()
            .get_commit(&commit_id)
            .map_err(|e| Error::Repository {
                message: format!("failed to get commit: {}", e),
            })?;

        let parent_hex = commit.parent_ids().first().map(|pid| pid.hex());

        Ok((parent_hex, commit_id.hex()))
    }

    /// Get structured log entries from the repository.
    pub fn log_entries(&mut self, limit: usize, all: bool) -> Result<Vec<LogEntry>> {
        let repo = self.load_repo_at_head()?;
        let workspace = self.workspace.as_ref().unwrap();

        let wc_commit_id = repo.view().get_wc_commit_id(workspace.workspace_name());

        let mut entries = Vec::new();
        let mut count = 0;

        // Collect all heads into a single traversal to avoid duplicates
        let mut to_visit: Vec<_> = repo.view().heads().iter().cloned().collect();
        let mut visited = std::collections::HashSet::new();

        while let Some(commit_id) = to_visit.pop() {
            if !all && count >= limit {
                break;
            }

            if !visited.insert(commit_id.clone()) {
                continue;
            }

            let commit = match repo.store().get_commit(&commit_id) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // Skip root commit
            if commit.change_id().hex().starts_with("zzzzzzzz") {
                continue;
            }

            let is_working_copy = wc_commit_id.map(|id| id == &commit_id).unwrap_or(false);

            let parent_change_ids: Vec<String> = commit
                .parent_ids()
                .iter()
                .filter_map(|pid| {
                    repo.store().get_commit(pid).ok().map(|p| {
                        let hex = p.change_id().hex();
                        if hex.len() > 8 {
                            hex[..8].to_string()
                        } else {
                            hex
                        }
                    })
                })
                .collect();

            let change_hex = commit.change_id().hex();
            let commit_hex = commit_id.hex();

            // Extract author timestamp as ISO 8601 string
            let author_sig = commit.author();
            let timestamp = {
                let millis = author_sig.timestamp.timestamp.0;
                let secs = millis / 1000;
                let tz_offset_mins = author_sig.timestamp.tz_offset;
                let tz_offset_secs = (tz_offset_mins as i64) * 60;
                let abs_offset = tz_offset_mins.unsigned_abs();
                let tz_sign = if tz_offset_mins >= 0 { '+' } else { '-' };
                let tz_hours = abs_offset / 60;
                let tz_mins = abs_offset % 60;
                let adjusted_secs = secs + tz_offset_secs;
                let days_since_epoch = adjusted_secs.div_euclid(86400);
                let time_of_day = adjusted_secs.rem_euclid(86400);
                let (year, month, day) = days_to_ymd(days_since_epoch);
                let hours = time_of_day / 3600;
                let minutes = (time_of_day % 3600) / 60;
                let seconds = time_of_day % 60;
                Some(format!(
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}{:02}:{:02}",
                    year, month, day, hours, minutes, seconds, tz_sign, tz_hours, tz_mins
                ))
            };

            // Extract author name, falling back to email
            let author = {
                let name = &author_sig.name;
                let email = &author_sig.email;
                if !name.is_empty() {
                    Some(name.clone())
                } else if !email.is_empty() {
                    Some(email.clone())
                } else {
                    None
                }
            };

            let full_commit_id = commit_hex.clone();

            entries.push(LogEntry {
                change_id: if change_hex.len() > 8 {
                    change_hex[..8].to_string()
                } else {
                    change_hex
                },
                commit_id: if commit_hex.len() > 8 {
                    commit_hex[..8].to_string()
                } else {
                    commit_hex
                },
                description: commit
                    .description()
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string(),
                parent_change_ids,
                is_working_copy,
                timestamp,
                author,
                full_commit_id,
            });

            count += 1;

            // Add parents to visit
            for parent_id in commit.parent_ids() {
                if !visited.contains(parent_id) {
                    to_visit.push(parent_id.clone());
                }
            }
        }

        Ok(entries)
    }

    /// Get operation log entries from the repository.
    pub fn operation_log(&mut self, limit: usize) -> Result<Vec<OperationInfo>> {
        let repo = self.load_repo_at_head()?;

        let mut operations = Vec::new();
        let mut current_op = Some(repo.operation().clone());
        let mut count = 0;

        while let Some(op) = current_op {
            if count >= limit {
                break;
            }

            operations.push(OperationInfo {
                id: op.id().hex(),
                description: op.metadata().description.clone(),
            });

            count += 1;

            // Get parent operation
            current_op = op.parents().next().and_then(|r| r.ok());
        }

        Ok(operations)
    }

    /// Restore the repository to a specific operation.
    pub fn restore_operation(&mut self, op_id: &str) -> Result<()> {
        let settings = create_minimal_settings()?;
        let store_factories = get_store_factories();
        let wc_factories = get_working_copy_factories();

        let workspace = Workspace::load(&settings, &self.root, &store_factories, &wc_factories)
            .map_err(|e| Error::Repository {
                message: format!("failed to load workspace: {}", e),
            })?;

        let repo = workspace
            .repo_loader()
            .load_at_head()
            .map_err(|e| Error::Repository {
                message: format!("failed to load repository: {}", e),
            })?;

        // Find the operation by ID
        let op_id_obj = jj_lib::op_store::OperationId::try_from_hex(op_id).ok_or_else(|| {
            Error::Repository {
                message: format!("invalid operation ID: {}", op_id),
            }
        })?;

        let target_op = workspace
            .repo_loader()
            .load_operation(&op_id_obj)
            .map_err(|e| Error::Repository {
                message: format!("failed to load operation: {}", e),
            })?;

        // Load repo at target operation
        let target_repo =
            workspace
                .repo_loader()
                .load_at(&target_op)
                .map_err(|e| Error::Repository {
                    message: format!("failed to load repository at operation: {}", e),
                })?;

        // Create a transaction to record the restore
        let mut tx = repo.start_transaction();

        // Merge in the target operation's view
        tx.repo_mut()
            .merge(&repo, &target_repo)
            .map_err(|e| Error::Repository {
                message: format!("failed to merge operation: {}", e),
            })?;

        // Commit the restore transaction
        tx.commit(format!("restore to operation {}", op_id))
            .map_err(|e| Error::Repository {
                message: format!("failed to commit restore: {}", e),
            })?;

        // Clear cached workspace
        self.workspace = None;

        Ok(())
    }

    /// Commit the working copy via jj-lib: snapshot, run invariants, commit
    /// transaction, export to git, and save TypedChange metadata.
    pub fn commit_working_copy(&mut self, opts: CommitOptions) -> Result<CommitResult> {
        let settings = create_minimal_settings()?;
        let store_factories = get_store_factories();
        let wc_factories = get_working_copy_factories();

        // Load fresh workspace (not cached — we need &mut for mutation)
        let mut workspace = Workspace::load(&settings, &self.root, &store_factories, &wc_factories)
            .map_err(|e| Error::Repository {
                message: format!("failed to load workspace: {}", e),
            })?;

        // Grab owned values before taking &mut workspace
        let workspace_name = workspace.workspace_name().to_owned();
        let repo = workspace
            .repo_loader()
            .load_at_head()
            .map_err(|e| Error::Repository {
                message: format!("failed to load repository: {}", e),
            })?;

        // Get WC commit and parent tree for diffing
        let wc_commit_id = repo
            .view()
            .get_wc_commit_id(&workspace_name)
            .cloned()
            .ok_or_else(|| Error::Repository {
                message: "no working copy commit found".into(),
            })?;

        let wc_commit = repo
            .store()
            .get_commit(&wc_commit_id)
            .map_err(|e| Error::Repository {
                message: format!("failed to get working copy commit: {}", e),
            })?;

        let parent_tree = wc_commit
            .parent_tree(&*repo)
            .map_err(|e| Error::Repository {
                message: format!("failed to get parent tree: {}", e),
            })?;

        // Snapshot the working copy to capture filesystem changes
        let mut locked_ws =
            workspace
                .start_working_copy_mutation()
                .map_err(|e| Error::Repository {
                    message: format!("failed to start working copy mutation: {}", e),
                })?;

        let snapshot_options = SnapshotOptions {
            base_ignores: load_base_ignores(&self.root),
            progress: None,
            start_tracking_matcher: &EverythingMatcher,
            force_tracking_matcher: &NothingMatcher,
            max_new_file_size: 1_000_000_000,
        };

        let (new_tree, _stats) = locked_ws
            .locked_wc()
            .snapshot(&snapshot_options)
            .block_on()
            .map_err(|e| Error::Repository {
                message: format!("failed to snapshot working copy: {}", e),
            })?;

        // Diff parent tree vs new tree to get files_changed
        let mut files_changed = Vec::new();
        let diff_iter =
            jj_lib::merged_tree::TreeDiffIterator::new(&parent_tree, &new_tree, &EverythingMatcher);
        for entry in diff_iter {
            files_changed.push(entry.path.as_internal_file_string().to_string());
        }

        // If nothing changed, bail early
        if files_changed.is_empty() {
            locked_ws
                .finish(repo.op_id().clone())
                .map_err(|e| Error::Repository {
                    message: format!("failed to finish working copy: {}", e),
                })?;
            return Err(Error::Repository {
                message: "nothing to commit - working tree clean".into(),
            });
        }

        // When --paths is specified, filter to only the requested paths and
        // build a selective tree containing just those changes.
        let commit_tree = if let Some(ref paths) = opts.paths {
            // Validate each requested path: must exist in the diff (changed)
            // or at least exist in new_tree (unchanged => skip silently).
            // If a path doesn't exist at all in the snapshot, error.
            for p in paths {
                if !files_changed.contains(p) {
                    let repo_path =
                        RepoPath::from_internal_string(p).map_err(|e| Error::Repository {
                            message: format!("invalid path '{}': {}", p, e),
                        })?;
                    let value = new_tree
                        .path_value(repo_path)
                        .map_err(|e| Error::Repository {
                            message: format!("failed to check path '{}': {}", p, e),
                        })?;
                    if value.is_absent() {
                        locked_ws
                            .finish(repo.op_id().clone())
                            .map_err(|e| Error::Repository {
                                message: format!("failed to finish working copy: {}", e),
                            })?;
                        return Err(Error::Repository {
                            message: format!(
                                "path '{}' does not exist in the working copy snapshot",
                                p
                            ),
                        });
                    }
                    // Path exists but is unchanged — skip silently (no-op)
                }
            }

            // Filter files_changed to only include paths in --paths
            files_changed.retain(|f| paths.iter().any(|p| f == p));

            if files_changed.is_empty() {
                locked_ws
                    .finish(repo.op_id().clone())
                    .map_err(|e| Error::Repository {
                        message: format!("failed to finish working copy: {}", e),
                    })?;
                return Err(Error::Repository {
                    message: "no changes in specified paths".into(),
                });
            }

            // Build a selective tree: start from parent_tree, overlay only the
            // requested paths from new_tree.
            let mut tree_builder = MergedTreeBuilder::new(parent_tree.clone());
            for path_str in &files_changed {
                let repo_path =
                    RepoPath::from_internal_string(path_str).map_err(|e| Error::Repository {
                        message: format!("invalid path '{}': {}", path_str, e),
                    })?;
                let new_value = new_tree
                    .path_value(repo_path)
                    .map_err(|e| Error::Repository {
                        message: format!("failed to read path '{}' from snapshot: {}", path_str, e),
                    })?;
                tree_builder.set_or_remove(repo_path.to_owned(), new_value);
            }
            tree_builder.write_tree().map_err(|e| Error::Repository {
                message: format!("failed to build selective tree: {}", e),
            })?
        } else {
            new_tree
        };

        // Run invariants between snapshot and commit (safe: no commit yet)
        let invariants = if opts.run_invariants && self.has_manifest() {
            match self.run_invariants(InvariantTrigger::PreCommit) {
                Ok(results) => results,
                Err((name, cmd, code, stdout, stderr)) => {
                    // Finish locked workspace before returning error (best-effort:
                    // if this fails the working copy may need manual recovery)
                    if let Err(e) = locked_ws.finish(repo.op_id().clone()) {
                        eprintln!("warning: failed to release working copy lock: {}", e);
                    }
                    return Err(Error::InvariantFailed {
                        name,
                        command: cmd,
                        exit_code: code,
                        stdout,
                        stderr,
                    });
                }
            }
        } else {
            HashMap::new()
        };

        // Start jj-lib transaction
        let mut tx = repo.start_transaction();

        // Rewrite WC commit with the (possibly selective) tree and commit message
        let committed = tx
            .repo_mut()
            .rewrite_commit(&wc_commit)
            .set_tree(commit_tree)
            .set_description(&opts.message)
            .write()
            .map_err(|e| Error::Repository {
                message: format!("failed to write commit: {}", e),
            })?;

        // Move the jj bookmark for the current git branch
        if let Some(branch_name) = get_current_git_branch(&self.root) {
            let ref_name: &jj_lib::ref_name::RefName = branch_name.as_str().as_ref();
            tx.repo_mut().set_local_bookmark_target(
                ref_name,
                jj_lib::op_store::RefTarget::normal(committed.id().clone()),
            );
        }

        // Create new empty WC commit (jj model: @ is always in-progress)
        if !opts.no_new {
            let new_wc_commit = tx
                .repo_mut()
                .new_commit(vec![committed.id().clone()], committed.tree())
                .write()
                .map_err(|e| Error::Repository {
                    message: format!("failed to create new working copy commit: {}", e),
                })?;
            tx.repo_mut()
                .set_wc_commit(workspace_name.clone(), new_wc_commit.id().clone())
                .map_err(|e| Error::Repository {
                    message: format!("failed to set working copy: {}", e),
                })?;
        } else {
            tx.repo_mut()
                .set_wc_commit(workspace_name.clone(), committed.id().clone())
                .map_err(|e| Error::Repository {
                    message: format!("failed to set working copy: {}", e),
                })?;
        }

        // Rebase any descendant commits
        tx.repo_mut()
            .rebase_descendants()
            .map_err(|e| Error::Repository {
                message: format!("failed to rebase descendants: {}", e),
            })?;

        // Export jj refs to git (syncs bookmarks → git branches)
        let _ = jj_lib::git::export_refs(tx.repo_mut());

        // Commit the transaction
        let new_repo = tx.commit("commit").map_err(|e| Error::Repository {
            message: format!("failed to commit transaction: {}", e),
        })?;

        // Persist working copy state
        locked_ws
            .finish(new_repo.op_id().clone())
            .map_err(|e| Error::Repository {
                message: format!("failed to finish working copy: {}", e),
            })?;

        // Sync git state directly (in colocated mode, jj detaches HEAD and
        // export_refs may not update the git branch in all scenarios)
        let commit_hex = committed.id().hex();
        if let Some(branch) = get_current_git_branch(&self.root) {
            // Move the git branch ref to the committed change
            let update_ref = Command::new("git")
                .current_dir(&self.root)
                .args(["update-ref", &format!("refs/heads/{}", branch), &commit_hex])
                .output();
            if let Err(e) = update_ref {
                eprintln!(
                    "warning: failed to update git ref for branch '{}': {}",
                    branch, e
                );
            }
            // Re-attach HEAD to the branch (jj colocated mode detaches HEAD)
            let symbolic_ref = Command::new("git")
                .current_dir(&self.root)
                .args(["symbolic-ref", "HEAD", &format!("refs/heads/{}", branch)])
                .output();
            if let Err(e) = symbolic_ref {
                eprintln!(
                    "warning: failed to set git HEAD to branch '{}': {}",
                    branch, e
                );
            }
        }

        // Save TypedChange metadata
        let mut typed_change =
            TypedChange::new(committed.change_id().hex(), opts.change_type, &opts.message)
                .with_files(files_changed.clone());

        if let Some(category) = opts.category {
            typed_change = typed_change.with_category(category);
        }
        if opts.breaking {
            typed_change = typed_change.breaking();
        }

        typed_change.invariants = InvariantsResult {
            checked: invariants.keys().cloned().collect(),
            status: if invariants.is_empty() {
                InvariantStatus::Skipped
            } else if invariants.values().all(|s| *s == InvariantStatus::Passed) {
                InvariantStatus::Passed
            } else {
                InvariantStatus::Failed
            },
            details: invariants.clone(),
        };

        self.save_typed_change(&typed_change)?;

        // Invalidate cached workspace
        self.workspace = None;

        let short_commit = if commit_hex.len() > 12 {
            &commit_hex[..12]
        } else {
            &commit_hex
        };

        Ok(CommitResult {
            change_id: committed.change_id().hex(),
            commit_id: short_commit.to_string(),
            operation_id: new_repo.op_id().hex(),
            files_changed,
            invariants,
        })
    }

    /// Get the raw ASCII graph output using git (no jj CLI dependency).
    pub fn log_ascii(&mut self, limit: usize, all: bool) -> Result<String> {
        let limit_str = limit.to_string();
        let mut args = vec!["log", "--graph", "--oneline", "--decorate"];

        if !all {
            args.push("-n");
            args.push(&limit_str);
        } else {
            args.push("--all");
        }

        let output = Command::new("git")
            .args(&args)
            .current_dir(&self.root)
            .output()
            .map_err(|e| Error::Repository {
                message: format!("failed to run git log: {}", e),
            })?;

        if !output.status.success() {
            return Err(Error::Repository {
                message: format!(
                    "git log failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

/// Convert days since Unix epoch to (year, month, day) using civil calendar arithmetic.
pub fn days_to_ymd(days: i64) -> (i64, u32, u32) {
    // Algorithm from Howard Hinnant's chrono-compatible date calculations
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month index [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Get the current git branch name. In jj colocated mode, HEAD may be
/// detached, so we fall back to checking the configured default branch
/// and then common branch names.
fn get_current_git_branch(root: &Path) -> Option<String> {
    // Try symbolic ref first (normal git state)
    let output = Command::new("git")
        .current_dir(root)
        .args(["symbolic-ref", "--short", "HEAD"])
        .output()
        .ok()?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !branch.is_empty() {
            return Some(branch);
        }
    }

    // Fallback for detached HEAD: check git config for default branch name
    let config_output = Command::new("git")
        .current_dir(root)
        .args(["config", "--get", "init.defaultBranch"])
        .output()
        .ok();
    if let Some(co) = config_output {
        if co.status.success() {
            let configured = String::from_utf8_lossy(&co.stdout).trim().to_string();
            if !configured.is_empty() {
                let verify = Command::new("git")
                    .current_dir(root)
                    .args([
                        "rev-parse",
                        "--verify",
                        &format!("refs/heads/{}", configured),
                    ])
                    .output()
                    .ok();
                if verify.map(|v| v.status.success()).unwrap_or(false) {
                    return Some(configured);
                }
            }
        }
    }

    // Last resort: check common default branch names
    for name in &["main", "master"] {
        let output = Command::new("git")
            .current_dir(root)
            .args(["rev-parse", "--verify", &format!("refs/heads/{}", name)])
            .output()
            .ok()?;
        if output.status.success() {
            return Some(name.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::change::ChangeType;
    use tempfile::TempDir;

    fn setup_test_repo() -> (TempDir, Repo) {
        let tmp = TempDir::new().unwrap();

        // Create .jj directory to simulate jj repo
        std::fs::create_dir(tmp.path().join(".jj")).unwrap();

        // Create .agent directory with manifest
        std::fs::create_dir_all(tmp.path().join(".agent")).unwrap();
        std::fs::write(
            tmp.path().join(".agent/manifest.toml"),
            r#"
[repo]
name = "test-repo"
"#,
        )
        .unwrap();

        let repo = Repo::open(tmp.path()).unwrap();
        (tmp, repo)
    }

    #[test]
    fn open_repo() {
        let (tmp, repo) = setup_test_repo();
        assert_eq!(repo.root(), tmp.path());
    }

    #[test]
    fn has_manifest() {
        let (_tmp, repo) = setup_test_repo();
        assert!(repo.has_manifest());
    }

    #[test]
    fn load_manifest() {
        let (_tmp, mut repo) = setup_test_repo();
        let manifest = repo.manifest().unwrap();
        assert_eq!(manifest.repo.name, "test-repo");
    }

    #[test]
    fn save_and_load_typed_change() {
        let (tmp, repo) = setup_test_repo();

        let change = TypedChange::new("testchange", ChangeType::Behavioral, "Test change");

        repo.save_typed_change(&change).unwrap();

        // Verify file was created
        assert!(tmp.path().join(".agent/changes/testchange.toml").exists());

        // Load it back
        let loaded = repo.get_typed_change("testchange").unwrap();
        assert_eq!(loaded.change_id, "testchange");
    }

    #[test]
    fn file_hash_precondition() {
        use crate::intent::{ChangeSpec, Intent, Preconditions};
        use sha2::{Digest, Sha256};

        let (tmp, mut repo) = setup_test_repo();

        // Create a test file
        let test_content = b"Hello, world!";
        std::fs::write(tmp.path().join("test.txt"), test_content).unwrap();

        // Calculate the correct hash
        let mut hasher = Sha256::new();
        hasher.update(test_content);
        let correct_hash = hex::encode(hasher.finalize());

        // Create intent with correct hash precondition
        let preconds = Preconditions::default().with_file_hash("test.txt", &correct_hash);
        let intent = Intent::new(
            "Test intent",
            ChangeType::Test,
            ChangeSpec::Files { operations: vec![] },
        )
        .with_preconditions(preconds);

        // Should pass
        let result = repo.check_preconditions(&intent);
        assert!(result.is_ok());

        // Now test with wrong hash
        let bad_preconds = Preconditions::default().with_file_hash("test.txt", "badhash");
        let bad_intent = Intent::new(
            "Test intent",
            ChangeType::Test,
            ChangeSpec::Files { operations: vec![] },
        )
        .with_preconditions(bad_preconds);

        let bad_result = repo.check_preconditions(&bad_intent);
        assert!(bad_result.is_err());

        match bad_result {
            Err(IntentResult::PreconditionFailed { reason, .. }) => {
                assert!(reason.contains("hash mismatch"));
            }
            _ => panic!("Expected PreconditionFailed"),
        }

        // Test uppercase hash works too (case-insensitive)
        let uppercase_hash = correct_hash.to_uppercase();
        let upper_preconds = Preconditions::default().with_file_hash("test.txt", &uppercase_hash);
        let upper_intent = Intent::new(
            "Test intent",
            ChangeType::Test,
            ChangeSpec::Files { operations: vec![] },
        )
        .with_preconditions(upper_preconds);

        let upper_result = repo.check_preconditions(&upper_intent);
        assert!(upper_result.is_ok(), "Uppercase hash should match");
    }

    #[test]
    fn days_to_ymd_unix_epoch() {
        let (y, m, d) = super::days_to_ymd(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn days_to_ymd_known_date() {
        // 2026-02-14 is 20498 days since epoch
        let (y, m, d) = super::days_to_ymd(20498);
        assert_eq!((y, m, d), (2026, 2, 14));
    }

    #[test]
    fn days_to_ymd_leap_year() {
        // 2000-02-29 is 11016 days since epoch
        let (y, m, d) = super::days_to_ymd(11016);
        assert_eq!((y, m, d), (2000, 2, 29));
    }

    #[test]
    fn log_entry_has_new_fields() {
        let entry = LogEntry {
            change_id: "abcd1234".to_string(),
            commit_id: "ef567890".to_string(),
            description: "test entry".to_string(),
            parent_change_ids: vec![],
            is_working_copy: false,
            timestamp: Some("2026-02-14T10:30:00+00:00".to_string()),
            author: Some("Test User".to_string()),
            full_commit_id: "ef567890abcdef1234567890abcdef1234567890".to_string(),
        };
        assert_eq!(
            entry.timestamp.as_deref(),
            Some("2026-02-14T10:30:00+00:00")
        );
        assert_eq!(entry.author.as_deref(), Some("Test User"));
        assert_eq!(entry.full_commit_id.len(), 40);
    }
}
