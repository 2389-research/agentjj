// ABOUTME: Manifest schema and parser for .agent/manifest.toml
// ABOUTME: Defines repo capabilities, interfaces, invariants, and permissions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::{Error, Result};

/// The root manifest structure, typically at `.agent/manifest.toml`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Manifest {
    pub repo: RepoInfo,

    #[serde(default)]
    pub entry_points: HashMap<String, String>,

    #[serde(default)]
    pub interfaces: HashMap<String, String>,

    #[serde(default)]
    pub invariants: HashMap<String, Invariant>,

    #[serde(default)]
    pub permissions: Permissions,

    #[serde(default)]
    pub branches: BranchConfig,

    #[serde(default)]
    pub review: ReviewConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepoInfo {
    pub name: String,

    #[serde(default)]
    pub description: String,

    #[serde(default)]
    pub languages: Vec<String>,

    #[serde(default = "default_vcs")]
    pub vcs: String,
}

fn default_vcs() -> String {
    "jj".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Invariant {
    /// Simple form: just a command string
    Simple(String),

    /// Full form: command with triggers
    Full {
        cmd: String,
        #[serde(default)]
        on: Vec<InvariantTrigger>,
    },
}

impl Invariant {
    pub fn command(&self) -> &str {
        match self {
            Invariant::Simple(cmd) => cmd,
            Invariant::Full { cmd, .. } => cmd,
        }
    }

    pub fn triggers(&self) -> &[InvariantTrigger] {
        match self {
            Invariant::Simple(_) => &[],
            Invariant::Full { on, .. } => on,
        }
    }

    pub fn should_run_on(&self, trigger: InvariantTrigger) -> bool {
        let triggers = self.triggers();
        triggers.is_empty() || triggers.contains(&trigger)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InvariantTrigger {
    PrePush,
    Pr,
    PreCommit,
    Always,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Permissions {
    #[serde(default)]
    pub allow_change: Vec<String>,

    #[serde(default)]
    pub deny_change: Vec<String>,

    #[serde(default)]
    pub allow_push: Vec<String>,

    #[serde(default)]
    pub deny_push: Vec<String>,
}

impl Permissions {
    /// Check if a path is allowed for changes (local modifications)
    pub fn can_change(&self, path: &str) -> bool {
        // Deny takes precedence
        if self.matches_any(path, &self.deny_change) {
            return false;
        }
        // If allow list is empty, allow everything not denied
        if self.allow_change.is_empty() {
            return true;
        }
        self.matches_any(path, &self.allow_change)
    }

    /// Check if a branch is allowed for push
    pub fn can_push(&self, branch: &str) -> bool {
        if self.matches_any(branch, &self.deny_push) {
            return false;
        }
        if self.allow_push.is_empty() {
            return true;
        }
        self.matches_any(branch, &self.allow_push)
    }

    fn matches_any(&self, path: &str, patterns: &[String]) -> bool {
        patterns.iter().any(|p| Self::glob_match(p, path))
    }

    fn glob_match(pattern: &str, path: &str) -> bool {
        // Simple glob matching: ** matches anything, * matches single segment
        if pattern == "**" {
            return true;
        }
        if pattern.contains("**") {
            let prefix = pattern.trim_end_matches("/**").trim_end_matches("**");
            return path.starts_with(prefix);
        }
        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                return path.starts_with(parts[0]) && path.ends_with(parts[1]);
            }
        }
        pattern == path
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchConfig {
    #[serde(default = "default_trunk")]
    pub trunk: String,

    #[serde(default)]
    pub protected: Vec<String>,
}

fn default_trunk() -> String {
    "main".to_string()
}

impl Default for BranchConfig {
    fn default() -> Self {
        Self {
            trunk: default_trunk(),
            protected: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReviewConfig {
    /// Paths that require human review before merge
    #[serde(default)]
    pub require_human: Vec<String>,
}

impl Manifest {
    pub const DEFAULT_PATH: &'static str = ".agent/manifest.toml";

    /// Load manifest from a file path
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|_| Error::ManifestNotFound {
            path: path.display().to_string(),
        })?;
        Self::parse(&content)
    }

    /// Load manifest from a repo root, looking for .agent/manifest.toml
    pub fn load_from_repo(repo_root: impl AsRef<Path>) -> Result<Self> {
        let path = repo_root.as_ref().join(Self::DEFAULT_PATH);
        Self::load(path)
    }

    /// Parse manifest from TOML string
    pub fn parse(content: &str) -> Result<Self> {
        toml::from_str(content).map_err(Into::into)
    }

    /// Serialize manifest to TOML string
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).map_err(|e| Error::ManifestParse {
            message: e.to_string(),
            line: None,
        })
    }

    /// Check if a path requires human review
    pub fn requires_human_review(&self, path: &str) -> bool {
        self.review
            .require_human
            .iter()
            .any(|p| Permissions::glob_match(p, path))
    }

    /// Get all invariants that should run for a given trigger
    pub fn invariants_for(&self, trigger: InvariantTrigger) -> Vec<(&str, &Invariant)> {
        self.invariants
            .iter()
            .filter(|(_, inv)| inv.should_run_on(trigger))
            .map(|(name, inv)| (name.as_str(), inv))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_MANIFEST: &str = r#"
[repo]
name = "payment-service"
description = "Handles payment processing"
languages = ["python"]

[entry_points]
cli = "src/cli.py:main"
api = "src/api.py:app"
tests = "pytest tests/"

[interfaces]
api_schema = "openapi.yaml"
events_schema = "schemas/events.json"

[invariants]
tests_pass = { cmd = "pytest -q", on = ["pre-push", "pr"] }
types_check = { cmd = "mypy src/", on = ["pre-push"] }
no_secrets = "! grep -r 'API_KEY=' src/"

[permissions]
allow_change = ["src/**", "tests/**"]
deny_change = [".agent/*", "migrations/*"]
allow_push = ["feat/*", "fix/*"]
deny_push = ["main", "release/*"]

[branches]
trunk = "main"
protected = ["main", "release/*"]

[review]
require_human = ["src/billing/*", "migrations/*"]
"#;

    #[test]
    fn parse_complete_manifest() {
        let manifest = Manifest::parse(SAMPLE_MANIFEST).unwrap();

        assert_eq!(manifest.repo.name, "payment-service");
        assert_eq!(manifest.repo.languages, vec!["python"]);
        assert_eq!(manifest.entry_points.get("cli").unwrap(), "src/cli.py:main");
        assert!(manifest.invariants.contains_key("tests_pass"));
    }

    #[test]
    fn permissions_allow_deny() {
        let manifest = Manifest::parse(SAMPLE_MANIFEST).unwrap();

        // Allowed paths
        assert!(manifest.permissions.can_change("src/api.py"));
        assert!(manifest.permissions.can_change("tests/test_api.py"));

        // Denied paths
        assert!(!manifest.permissions.can_change(".agent/manifest.toml"));
        assert!(!manifest.permissions.can_change("migrations/001.sql"));
    }

    #[test]
    fn branch_permissions() {
        let manifest = Manifest::parse(SAMPLE_MANIFEST).unwrap();

        assert!(manifest.permissions.can_push("feat/add-retry"));
        assert!(manifest.permissions.can_push("fix/bug-123"));
        assert!(!manifest.permissions.can_push("main"));
        assert!(!manifest.permissions.can_push("release/v1.0"));
    }

    #[test]
    fn invariant_triggers() {
        let manifest = Manifest::parse(SAMPLE_MANIFEST).unwrap();

        let pre_push = manifest.invariants_for(InvariantTrigger::PrePush);
        let names: Vec<_> = pre_push.iter().map(|(n, _)| *n).collect();

        assert!(names.contains(&"tests_pass"));
        assert!(names.contains(&"types_check"));

        // no_secrets has no triggers, so it runs always
        assert!(names.contains(&"no_secrets"));
    }

    #[test]
    fn human_review_required() {
        let manifest = Manifest::parse(SAMPLE_MANIFEST).unwrap();

        assert!(manifest.requires_human_review("src/billing/processor.py"));
        assert!(manifest.requires_human_review("migrations/002.sql"));
        assert!(!manifest.requires_human_review("src/api.py"));
    }

    #[test]
    fn roundtrip_toml() {
        let original = Manifest::parse(SAMPLE_MANIFEST).unwrap();
        let toml_str = original.to_toml().unwrap();
        let reparsed = Manifest::parse(&toml_str).unwrap();

        assert_eq!(original.repo.name, reparsed.repo.name);
        assert_eq!(
            original.permissions.allow_change,
            reparsed.permissions.allow_change
        );
    }

    #[test]
    fn minimal_manifest() {
        let minimal = r#"
[repo]
name = "tiny"
"#;
        let manifest = Manifest::parse(minimal).unwrap();
        assert_eq!(manifest.repo.name, "tiny");
        assert_eq!(manifest.branches.trunk, "main"); // default
        assert!(manifest.invariants.is_empty());
    }
}
