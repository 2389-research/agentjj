// ABOUTME: Typed change metadata for jj changes
// ABOUTME: Semantic change records keyed by jj change ID (stable across rebases)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::{Error, Result};

/// Semantic type of the change
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    /// Changes behavior (new feature, bug fix)
    Behavioral,
    /// Restructures code without changing behavior
    Refactor,
    /// Modifies schemas, types, or interfaces
    Schema,
    /// Documentation only
    Docs,
    /// Dependency updates
    Deps,
    /// Configuration changes
    Config,
    /// Test additions or modifications
    Test,
}

/// Category of the change (more granular than type)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeCategory {
    Feature,
    Fix,
    Perf,
    Security,
    Breaking,
    Deprecation,
    Chore,
}

/// Typed metadata for a jj change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedChange {
    /// The jj change ID (stable across rebases)
    pub change_id: String,

    /// Semantic type of the change
    #[serde(rename = "type")]
    pub change_type: ChangeType,

    /// Category for more granular classification
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<ChangeCategory>,

    /// Human-readable intent description
    pub intent: String,

    /// Files modified in this change
    #[serde(default)]
    pub files: Vec<String>,

    /// Whether this is a breaking change
    #[serde(default)]
    pub breaking: bool,

    /// Dependencies added in this change
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies_added: Vec<String>,

    /// Dependencies removed in this change
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies_removed: Vec<String>,

    /// Invariants that were checked
    #[serde(default)]
    pub invariants: InvariantsResult,

    /// Additional structured metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InvariantsResult {
    /// Names of invariants that were checked
    #[serde(default)]
    pub checked: Vec<String>,

    /// Overall status
    #[serde(default)]
    pub status: InvariantStatus,

    /// Per-invariant results (if any failed)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub details: HashMap<String, InvariantStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum InvariantStatus {
    #[default]
    Unknown,
    Passed,
    Failed,
    Skipped,
}

impl TypedChange {
    /// Create a new typed change
    pub fn new(change_id: impl Into<String>, change_type: ChangeType, intent: impl Into<String>) -> Self {
        Self {
            change_id: change_id.into(),
            change_type,
            category: None,
            intent: intent.into(),
            files: Vec::new(),
            breaking: false,
            dependencies_added: Vec::new(),
            dependencies_removed: Vec::new(),
            invariants: InvariantsResult::default(),
            metadata: HashMap::new(),
        }
    }

    /// Set the category
    pub fn with_category(mut self, category: ChangeCategory) -> Self {
        self.category = Some(category);
        self
    }

    /// Set files changed
    pub fn with_files(mut self, files: Vec<String>) -> Self {
        self.files = files;
        self
    }

    /// Mark as breaking change
    pub fn breaking(mut self) -> Self {
        self.breaking = true;
        self
    }

    /// Storage path for this change's metadata
    pub fn storage_path(&self) -> String {
        format!(".agent/changes/{}.toml", self.change_id)
    }

    /// Load typed change from file
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())?;
        Self::parse(&content)
    }

    /// Load typed change by change ID from repo
    pub fn load_from_repo(repo_root: impl AsRef<Path>, change_id: &str) -> Result<Self> {
        let path = repo_root.as_ref().join(format!(".agent/changes/{}.toml", change_id));
        if !path.exists() {
            return Err(Error::ChangeNotFound {
                change_id: change_id.to_string(),
            });
        }
        Self::load(path)
    }

    /// Parse from TOML
    pub fn parse(content: &str) -> Result<Self> {
        toml::from_str(content).map_err(Into::into)
    }

    /// Serialize to TOML
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).map_err(|e| Error::ManifestParse {
            message: e.to_string(),
            line: None,
        })
    }

    /// Save to file
    pub fn save(&self, repo_root: impl AsRef<Path>) -> Result<()> {
        let path = repo_root.as_ref().join(self.storage_path());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, self.to_toml()?)?;
        Ok(())
    }
}

/// Index of all typed changes in a repo
#[derive(Debug, Default)]
pub struct ChangeIndex {
    changes: HashMap<String, TypedChange>,
}

impl ChangeIndex {
    /// Load all typed changes from a repo
    pub fn load_from_repo(repo_root: impl AsRef<Path>) -> Result<Self> {
        let changes_dir = repo_root.as_ref().join(".agent/changes");
        let mut changes = HashMap::new();

        if changes_dir.exists() {
            for entry in std::fs::read_dir(&changes_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map(|e| e == "toml").unwrap_or(false) {
                    if let Ok(change) = TypedChange::load(&path) {
                        changes.insert(change.change_id.clone(), change);
                    }
                }
            }
        }

        Ok(Self { changes })
    }

    /// Get a change by ID
    pub fn get(&self, change_id: &str) -> Option<&TypedChange> {
        self.changes.get(change_id)
    }

    /// Get all changes of a given type
    pub fn by_type(&self, change_type: ChangeType) -> Vec<&TypedChange> {
        self.changes
            .values()
            .filter(|c| c.change_type == change_type)
            .collect()
    }

    /// Get all breaking changes
    pub fn breaking_changes(&self) -> Vec<&TypedChange> {
        self.changes.values().filter(|c| c.breaking).collect()
    }

    /// Get all changes
    pub fn all(&self) -> Vec<&TypedChange> {
        self.changes.values().collect()
    }

    /// Insert a change
    pub fn insert(&mut self, change: TypedChange) {
        self.changes.insert(change.change_id.clone(), change);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_typed_change() {
        let change = TypedChange::new("qpvuntsm", ChangeType::Behavioral, "Add retry logic")
            .with_category(ChangeCategory::Feature)
            .with_files(vec!["src/webhook.py".into(), "tests/test_webhook.py".into()]);

        assert_eq!(change.change_id, "qpvuntsm");
        assert_eq!(change.change_type, ChangeType::Behavioral);
        assert_eq!(change.category, Some(ChangeCategory::Feature));
        assert_eq!(change.files.len(), 2);
    }

    #[test]
    fn roundtrip_toml() {
        let change = TypedChange::new("qpvuntsm", ChangeType::Refactor, "Clean up imports")
            .with_files(vec!["src/api.py".into()]);

        let toml = change.to_toml().unwrap();
        let reparsed = TypedChange::parse(&toml).unwrap();

        assert_eq!(change.change_id, reparsed.change_id);
        assert_eq!(change.intent, reparsed.intent);
    }

    #[test]
    fn parse_from_toml() {
        let toml = r#"
change_id = "kkmpptqz"
type = "schema"
intent = "Add user_id field to events"
files = ["schemas/events.json", "src/models.py"]
breaking = true

[invariants]
checked = ["tests_pass", "types_check"]
status = "passed"
"#;

        let change = TypedChange::parse(toml).unwrap();
        assert_eq!(change.change_type, ChangeType::Schema);
        assert!(change.breaking);
        assert_eq!(change.invariants.status, InvariantStatus::Passed);
    }

    #[test]
    fn storage_path() {
        let change = TypedChange::new("abc123", ChangeType::Docs, "Update readme");
        assert_eq!(change.storage_path(), ".agent/changes/abc123.toml");
    }
}
