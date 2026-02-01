// ABOUTME: Repository operations wrapping the jj CLI
// ABOUTME: Provides high-level operations for agent workflows via subprocess

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::change::{InvariantStatus, InvariantsResult, TypedChange};
use crate::error::{ConflictDetail, Error, Result};
use crate::intent::{ChangeSpec, FileOperation, Intent, IntentResult};
use crate::manifest::{InvariantTrigger, Manifest};

/// A repository handle for agent operations
pub struct Repo {
    /// Path to the repository root
    root: PathBuf,
    /// Cached manifest (loaded lazily)
    manifest: Option<Manifest>,
}

/// Output from `jj log` with template (for future JSON parsing)
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JjLogEntry {
    change_id: String,
    commit_id: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    empty: bool,
    #[serde(default)]
    conflict: bool,
}

/// Output from `jj op log` (for future JSON parsing)
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JjOpLogEntry {
    id: String,
}

impl Repo {
    /// Open a repository at the given path
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let root = path.as_ref().to_path_buf();
        Ok(Self {
            root,
            manifest: None,
        })
    }

    /// Discover and open a repository from the current directory or ancestors
    pub fn discover() -> Result<Self> {
        let cwd = std::env::current_dir()?;
        let mut current = cwd.as_path();

        loop {
            if current.join(".jj").exists() {
                return Self::open(current);
            }
            match current.parent() {
                Some(parent) => current = parent,
                None => {
                    return Err(Error::Repository {
                        message: "not a jj repository (or any parent)".into(),
                    })
                }
            }
        }
    }

    /// Get the repository root path
    pub fn root(&self) -> &Path {
        &self.root
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

    /// Run a jj command and return stdout
    fn jj(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("jj")
            .args(args)
            .current_dir(&self.root)
            .output()
            .map_err(|e| Error::Repository {
                message: format!("failed to run jj: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Repository {
                message: format!("jj {} failed: {}", args.join(" "), stderr),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Run a jj command, allowing failure (returns None on error)
    fn jj_maybe(&self, args: &[&str]) -> Option<String> {
        self.jj(args).ok()
    }

    /// Get the current change ID (@ in jj)
    pub fn current_change_id(&self) -> Result<String> {
        let output = self.jj(&[
            "log",
            "-r",
            "@",
            "--no-graph",
            "-T",
            r#"change_id ++ "\n""#,
        ])?;
        Ok(output.trim().to_string())
    }

    /// Get the current commit ID
    pub fn current_commit_id(&self) -> Result<String> {
        let output = self.jj(&[
            "log",
            "-r",
            "@",
            "--no-graph",
            "-T",
            r#"commit_id ++ "\n""#,
        ])?;
        Ok(output.trim().to_string())
    }

    /// Get current operation ID
    pub fn current_operation_id(&self) -> Result<String> {
        let output = self.jj(&["op", "log", "--no-graph", "-T", r#"id ++ "\n""#, "--limit", "1"])?;
        Ok(output.trim().to_string())
    }

    /// Read file content at a specific change or branch
    pub fn read_file(&self, path: &str, at: Option<&str>) -> Result<String> {
        let rev = at.unwrap_or("@");
        self.jj(&["file", "show", "-r", rev, path])
    }

    /// List files changed in a specific change
    pub fn changed_files(&self, change_id: &str) -> Result<Vec<String>> {
        let output = self.jj(&["diff", "-r", change_id, "--summary"])?;
        let files: Vec<String> = output
            .lines()
            .filter_map(|line| {
                // Format is "M path" or "A path" or "D path"
                let parts: Vec<&str> = line.splitn(2, ' ').collect();
                if parts.len() == 2 {
                    Some(parts[1].to_string())
                } else {
                    None
                }
            })
            .collect();
        Ok(files)
    }

    /// Check if a branch/bookmark exists and get its change ID
    pub fn branch_change_id(&self, branch: &str) -> Result<Option<String>> {
        let output = self.jj_maybe(&[
            "log",
            "-r",
            branch,
            "--no-graph",
            "-T",
            r#"change_id ++ "\n""#,
        ]);
        Ok(output.map(|s| s.trim().to_string()))
    }

    /// Check if a change has conflicts
    pub fn has_conflicts(&self, change_id: &str) -> Result<bool> {
        let output = self.jj(&[
            "log",
            "-r",
            change_id,
            "--no-graph",
            "-T",
            r#"if(conflict, "true", "false")"#,
        ])?;
        Ok(output.trim() == "true")
    }

    /// Get conflict details for a change
    pub fn get_conflicts(&self, change_id: &str) -> Result<Vec<ConflictDetail>> {
        let output = self.jj(&["resolve", "-r", change_id, "--list"])?;
        let conflicts: Vec<ConflictDetail> = output
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| ConflictDetail {
                file: line.to_string(),
                ours: String::new(),   // TODO: extract actual content
                theirs: String::new(), // TODO: extract actual content
                base: None,
            })
            .collect();
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

        // 3. Create a new change
        self.jj(&["new", "-m", &intent.description])?;
        let change_id = self.current_change_id()?;
        let operation_id = self.current_operation_id()?;

        // 4. Apply changes
        let files_changed = match self.apply_changes(&intent.changes) {
            Ok(files) => files,
            Err(e) => {
                // Rollback on error
                let _ = self.jj(&["undo"]);
                return Err(e);
            }
        };

        // 5. Check for conflicts
        if self.has_conflicts(&change_id)? {
            let conflicts = self.get_conflicts(&change_id)?;
            return Ok(IntentResult::Conflict {
                change_id,
                operation_id: operation_id.clone(),
                conflicts,
                rollback_command: format!("jj op restore {}", self.get_previous_op_id()?),
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
                    return Ok(IntentResult::InvariantFailed {
                        invariant: name,
                        command: cmd,
                        exit_code: code,
                        stdout,
                        stderr,
                        change_id,
                        rollback_command: format!("jj op restore {}", self.get_previous_op_id()?),
                    });
                }
            }
        } else {
            HashMap::new()
        };

        // 8. Save typed change metadata
        let typed_change = TypedChange::new(change_id.clone(), intent.change_type, &intent.description)
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

    /// Check preconditions for an intent
    fn check_preconditions(&self, intent: &Intent) -> std::result::Result<(), IntentResult> {
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

                // Get changed files from jj
                self.jj(&["status"])?; // Ensure jj sees the changes
                let change_id = self.current_change_id()?;
                self.changed_files(&change_id)
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
    fn get_previous_op_id(&self) -> Result<String> {
        let output = self.jj(&["op", "log", "--no-graph", "-T", r#"id ++ "\n""#, "--limit", "2"])?;
        let lines: Vec<&str> = output.lines().collect();
        if lines.len() >= 2 {
            Ok(lines[1].trim().to_string())
        } else {
            Ok(lines.first().map(|s| s.trim().to_string()).unwrap_or_default())
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
    pub fn describe(&self, message: &str) -> Result<()> {
        self.jj(&["describe", "-m", message])?;
        Ok(())
    }

    /// Create a new change
    pub fn new_change(&self, message: Option<&str>) -> Result<String> {
        match message {
            Some(m) => self.jj(&["new", "-m", m])?,
            None => self.jj(&["new"])?,
        };
        self.current_change_id()
    }

    /// Squash changes into parent
    pub fn squash(&self) -> Result<()> {
        self.jj(&["squash"])?;
        Ok(())
    }
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

        let (tmp, repo) = setup_test_repo();

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
}
