// ABOUTME: Intent-based transactions for atomic agent operations
// ABOUTME: Single-operation interface with preconditions and structured results

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::change::{ChangeCategory, ChangeType, InvariantStatus};
use crate::error::ConflictDetail;

/// An intent to make changes to the repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    /// Human-readable description of what this intent does
    pub description: String,

    /// Semantic type of the change
    #[serde(rename = "type")]
    pub change_type: ChangeType,

    /// Category for more granular classification
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<ChangeCategory>,

    /// Preconditions that must be met
    #[serde(default)]
    pub preconditions: Preconditions,

    /// The changes to apply
    pub changes: ChangeSpec,

    /// Whether to run invariants before completing
    #[serde(default = "default_true")]
    pub run_invariants: bool,

    /// Whether this is a breaking change
    #[serde(default)]
    pub breaking: bool,
}

fn default_true() -> bool {
    true
}

/// Preconditions that must be satisfied before applying changes
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Preconditions {
    /// Expected operation ID (from jj op log)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,

    /// Expected change ID that a branch should point to
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub branch_at: HashMap<String, String>,

    /// Expected file hashes (sha256)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub file_hashes: HashMap<String, String>,

    /// Files that must exist
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_exist: Vec<String>,

    /// Files that must not exist
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_absent: Vec<String>,
}

impl Preconditions {
    pub fn is_empty(&self) -> bool {
        self.operation_id.is_none()
            && self.branch_at.is_empty()
            && self.file_hashes.is_empty()
            && self.files_exist.is_empty()
            && self.files_absent.is_empty()
    }

    /// Require a specific operation ID
    pub fn with_operation(mut self, op_id: impl Into<String>) -> Self {
        self.operation_id = Some(op_id.into());
        self
    }

    /// Require a branch to point to a specific change
    pub fn with_branch_at(
        mut self,
        branch: impl Into<String>,
        change_id: impl Into<String>,
    ) -> Self {
        self.branch_at.insert(branch.into(), change_id.into());
        self
    }

    /// Require a file to have a specific hash
    pub fn with_file_hash(mut self, path: impl Into<String>, hash: impl Into<String>) -> Self {
        self.file_hashes.insert(path.into(), hash.into());
        self
    }
}

/// Specification of changes to apply
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "format", rename_all = "lowercase")]
pub enum ChangeSpec {
    /// A unified diff patch
    Patch { content: String },

    /// Direct file operations
    Files { operations: Vec<FileOperation> },

    /// Reference to a file containing the patch
    PatchFile { path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum FileOperation {
    /// Create a new file
    Create { path: String, content: String },

    /// Replace file contents entirely
    Replace { path: String, content: String },

    /// Delete a file
    Delete { path: String },

    /// Rename/move a file
    Rename { from: String, to: String },
}

/// Result of applying an intent
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum IntentResult {
    /// Intent was applied successfully
    Success {
        /// The jj change ID created
        change_id: String,
        /// The jj operation ID
        operation_id: String,
        /// Files that were modified
        files_changed: Vec<String>,
        /// Invariant results
        invariants: HashMap<String, InvariantStatus>,
        /// PR URL if pushed and PR created
        #[serde(skip_serializing_if = "Option::is_none")]
        pr_url: Option<String>,
    },

    /// A precondition was not met
    PreconditionFailed {
        /// Which precondition failed
        reason: String,
        /// What was expected
        expected: String,
        /// What was found
        actual: String,
    },

    /// Changes conflicted with existing state
    Conflict {
        /// The change ID that was created (with conflicts)
        change_id: String,
        /// The operation ID (for rollback)
        operation_id: String,
        /// Details of each conflict
        conflicts: Vec<ConflictDetail>,
        /// Command to rollback
        rollback_command: String,
    },

    /// An invariant check failed
    InvariantFailed {
        /// Which invariant failed
        invariant: String,
        /// The command that was run
        command: String,
        /// Exit code
        exit_code: i32,
        /// Stdout from the command
        stdout: String,
        /// Stderr from the command
        stderr: String,
        /// The change ID (changes were applied but not finalized)
        change_id: String,
        /// Command to rollback
        rollback_command: String,
    },

    /// Permission was denied by manifest
    PermissionDenied {
        /// What action was denied
        action: String,
        /// What path triggered the denial
        path: String,
        /// Relevant manifest rule
        rule: String,
    },

    /// Requires human review per manifest
    RequiresReview {
        /// The change ID (created but not pushed)
        change_id: String,
        /// Paths that require review
        paths: Vec<String>,
        /// Message for the human reviewer
        message: String,
    },
}

impl IntentResult {
    /// Check if the result is a success
    pub fn is_success(&self) -> bool {
        matches!(self, IntentResult::Success { .. })
    }

    /// Get the change ID if available
    pub fn change_id(&self) -> Option<&str> {
        match self {
            IntentResult::Success { change_id, .. } => Some(change_id),
            IntentResult::Conflict { change_id, .. } => Some(change_id),
            IntentResult::InvariantFailed { change_id, .. } => Some(change_id),
            IntentResult::RequiresReview { change_id, .. } => Some(change_id),
            _ => None,
        }
    }

    /// Get rollback command if available
    pub fn rollback_command(&self) -> Option<&str> {
        match self {
            IntentResult::Conflict {
                rollback_command, ..
            } => Some(rollback_command),
            IntentResult::InvariantFailed {
                rollback_command, ..
            } => Some(rollback_command),
            _ => None,
        }
    }
}

impl Intent {
    /// Create a new intent
    pub fn new(
        description: impl Into<String>,
        change_type: ChangeType,
        changes: ChangeSpec,
    ) -> Self {
        Self {
            description: description.into(),
            change_type,
            category: None,
            preconditions: Preconditions::default(),
            changes,
            run_invariants: true,
            breaking: false,
        }
    }

    /// Add a category
    pub fn with_category(mut self, category: ChangeCategory) -> Self {
        self.category = Some(category);
        self
    }

    /// Set preconditions
    pub fn with_preconditions(mut self, preconditions: Preconditions) -> Self {
        self.preconditions = preconditions;
        self
    }

    /// Disable invariant checking
    pub fn skip_invariants(mut self) -> Self {
        self.run_invariants = false;
        self
    }

    /// Mark as breaking change
    pub fn breaking(mut self) -> Self {
        self.breaking = true;
        self
    }

    /// Serialize to JSON (for CLI output)
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    /// Parse from JSON
    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_intent_with_patch() {
        let intent = Intent::new(
            "Add retry logic to webhook handler",
            ChangeType::Behavioral,
            ChangeSpec::Patch {
                content: "--- a/src/webhook.py\n+++ b/src/webhook.py\n@@ -1 +1 @@\n-old\n+new"
                    .into(),
            },
        )
        .with_category(ChangeCategory::Feature)
        .with_preconditions(Preconditions::default().with_branch_at("main", "qpvuntsm"));

        assert_eq!(intent.change_type, ChangeType::Behavioral);
        assert!(intent.preconditions.branch_at.contains_key("main"));
    }

    #[test]
    fn create_intent_with_file_ops() {
        let intent = Intent::new(
            "Add new configuration file",
            ChangeType::Config,
            ChangeSpec::Files {
                operations: vec![FileOperation::Create {
                    path: "config/new.toml".into(),
                    content: "[settings]\nkey = \"value\"".into(),
                }],
            },
        );

        if let ChangeSpec::Files { operations } = &intent.changes {
            assert_eq!(operations.len(), 1);
        } else {
            panic!("Expected Files variant");
        }
    }

    #[test]
    fn intent_result_success() {
        let result = IntentResult::Success {
            change_id: "abc123".into(),
            operation_id: "op456".into(),
            files_changed: vec!["src/api.py".into()],
            invariants: [("tests_pass".into(), InvariantStatus::Passed)].into(),
            pr_url: Some("https://github.com/org/repo/pull/42".into()),
        };

        assert!(result.is_success());
        assert_eq!(result.change_id(), Some("abc123"));
    }

    #[test]
    fn intent_result_conflict() {
        let result = IntentResult::Conflict {
            change_id: "abc123".into(),
            operation_id: "op456".into(),
            conflicts: vec![ConflictDetail {
                file: "src/api.py".into(),
                ours: "fn a()".into(),
                theirs: "fn b()".into(),
                base: Some("fn orig()".into()),
            }],
            rollback_command: "jj op restore op455".into(),
        };

        assert!(!result.is_success());
        assert_eq!(result.change_id(), Some("abc123"));
        assert_eq!(result.rollback_command(), Some("jj op restore op455"));
    }

    #[test]
    fn serialize_intent_result_json() {
        let result = IntentResult::PreconditionFailed {
            reason: "branch has advanced".into(),
            expected: "qpvuntsm".into(),
            actual: "kkmpptqz".into(),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("precondition_failed"));
        assert!(json.contains("branch has advanced"));
    }

    #[test]
    fn preconditions_empty() {
        let empty = Preconditions::default();
        assert!(empty.is_empty());

        let with_op = Preconditions::default().with_operation("op123");
        assert!(!with_op.is_empty());
    }
}
