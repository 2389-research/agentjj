// ABOUTME: Integration tests for the agentjj CLI interface
// ABOUTME: Tests command-line behavior, exit codes, and JSON output format

use predicates::prelude::*;
use std::process::Command;
use tempfile::TempDir;

/// Helper to get a Command for the agentjj binary.
/// Uses the crate target dir to locate the binary built by cargo.
#[allow(deprecated)]
fn agentjj() -> assert_cmd::Command {
    assert_cmd::Command::cargo_bin("agentjj").unwrap()
}

/// Helper to create a temporary git repository for tests that require one.
/// agentjj auto-colocates with git repos, so we use git init.
fn setup_temp_jj_repo() -> Option<TempDir> {
    let tmp = TempDir::new().ok()?;

    // Initialize a git repository - agentjj will auto-colocate
    let status = Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .status()
        .ok()?;

    // Configure git user for commits
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(tmp.path())
        .status()
        .ok()?;

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(tmp.path())
        .status()
        .ok()?;

    if !status.success() {
        return None;
    }

    // Create a simple file so there's some content
    std::fs::write(tmp.path().join("README.md"), "# Test Repository\n").ok()?;

    Some(tmp)
}

// =============================================================================
// Test 1: --help returns success and shows usage
// =============================================================================

#[test]
fn help_returns_success_and_shows_usage() {
    agentjj()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Agent-oriented porcelain for Jujutsu",
        ))
        .stdout(predicate::str::contains("Usage:"))
        .stdout(predicate::str::contains("Commands:"));
}

#[test]
fn help_shows_subcommands() {
    agentjj()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("status"))
        .stdout(predicate::str::contains("orient"))
        .stdout(predicate::str::contains("schema"))
        .stdout(predicate::str::contains("skill"))
        .stdout(predicate::str::contains("quickstart"));
}

// =============================================================================
// Test 2: --version returns success
// =============================================================================

#[test]
fn version_returns_success() {
    agentjj()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("agentjj"));
}

// =============================================================================
// Test 3: status in non-repo directory fails with appropriate error
// =============================================================================

#[test]
fn status_in_non_repo_fails() {
    let tmp = TempDir::new().unwrap();

    agentjj()
        .arg("status")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("No git or jj repository found"));
}

// =============================================================================
// Test 4: --json status in non-repo returns JSON error format
// =============================================================================

#[test]
fn json_status_in_non_repo_returns_json_error() {
    let tmp = TempDir::new().unwrap();

    let output = agentjj()
        .args(["--json", "status"])
        .current_dir(tmp.path())
        .assert()
        .failure();

    // Get stdout and verify it's valid JSON with error structure
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);

    // Parse as JSON to validate format
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    assert_eq!(json["error"], true, "JSON should have error: true");
    assert!(
        json["message"].as_str().is_some(),
        "JSON should have message field"
    );
    assert!(
        json["message"]
            .as_str()
            .unwrap()
            .contains("No git or jj repository found"),
        "Error message should mention jj repository"
    );
}

// =============================================================================
// Test 5: orient basic functionality (requires temp jj repo)
// =============================================================================

#[test]
fn orient_in_jj_repo_succeeds() {
    let Some(tmp) = setup_temp_jj_repo() else {
        eprintln!("Skipping test: jj not available");
        return;
    };

    agentjj()
        .arg("orient")
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Repository Orientation").or(
            // In text mode it shows "Repository Orientation"
            // but we also accept if it just runs without error
            predicate::str::contains("Current change"),
        ));
}

#[test]
fn orient_json_returns_valid_structure() {
    let Some(tmp) = setup_temp_jj_repo() else {
        eprintln!("Skipping test: jj not available");
        return;
    };

    let output = agentjj()
        .args(["--json", "orient"])
        .current_dir(tmp.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);

    // Parse as JSON to validate format
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    // Verify expected structure
    assert!(
        json["current_state"].is_object(),
        "Should have current_state object"
    );
    assert!(
        json["current_state"]["change_id"].is_string(),
        "Should have change_id"
    );
    assert!(
        json["capabilities"].is_object(),
        "Should have capabilities object"
    );
    assert!(
        json["quick_start"].is_object(),
        "Should have quick_start object"
    );
}

#[test]
fn orient_in_non_repo_fails() {
    let tmp = TempDir::new().unwrap();

    agentjj()
        .arg("orient")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("No git or jj repository found"));
}

// =============================================================================
// Test 6: schema returns valid JSON listing all schemas
// =============================================================================

#[test]
fn schema_returns_success() {
    // Schema command doesn't require a jj repo
    agentjj()
        .arg("schema")
        .assert()
        .success()
        .stdout(predicate::str::contains("Available schemas"));
}

#[test]
fn schema_json_returns_valid_json() {
    let output = agentjj().args(["--json", "schema"]).assert().success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);

    // Parse as JSON to validate
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Schema output should be valid JSON");

    // Should be an object with schema definitions
    assert!(json.is_object(), "Schema should return a JSON object");

    // Verify expected schemas are present
    assert!(json["status"].is_object(), "Should have status schema");
    assert!(json["symbol"].is_object(), "Should have symbol schema");
    assert!(json["context"].is_object(), "Should have context schema");
    assert!(
        json["apply_result"].is_object(),
        "Should have apply_result schema"
    );
    assert!(json["error"].is_object(), "Should have error schema");
    assert!(json["orient"].is_object(), "Should have orient schema");
}

#[test]
fn schema_type_filter_works() {
    let output = agentjj()
        .args(["--json", "schema", "--type", "status"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Schema output should be valid JSON");

    // Should return just the status schema
    assert!(
        json["type"].is_string(),
        "Status schema should have type field"
    );
    assert!(
        json["properties"].is_object(),
        "Status schema should have properties"
    );
}

#[test]
fn schema_invalid_type_fails() {
    agentjj()
        .args(["schema", "--type", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown type"));
}

// =============================================================================
// Additional CLI behavior tests
// =============================================================================

#[test]
fn json_flag_is_global() {
    // --json can appear before the subcommand
    let tmp = TempDir::new().unwrap();

    let output = agentjj()
        .args(["--json", "status"])
        .current_dir(tmp.path())
        .assert()
        .failure();

    // Even on failure, output should be JSON
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let json: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
    assert!(
        json.is_ok(),
        "Error output with --json should be valid JSON"
    );
}

#[test]
fn status_in_jj_repo_succeeds() {
    let Some(tmp) = setup_temp_jj_repo() else {
        eprintln!("Skipping test: jj not available");
        return;
    };

    agentjj()
        .arg("status")
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Change:"));
}

#[test]
fn status_json_returns_valid_structure() {
    let Some(tmp) = setup_temp_jj_repo() else {
        eprintln!("Skipping test: jj not available");
        return;
    };

    let output = agentjj()
        .args(["--json", "status"])
        .current_dir(tmp.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Status output should be valid JSON");

    // Verify expected fields
    assert!(json["change_id"].is_string(), "Should have change_id");
    assert!(json["operation_id"].is_string(), "Should have operation_id");
    assert!(
        json["files_changed"].is_array(),
        "Should have files_changed array"
    );
    assert!(
        json["has_manifest"].is_boolean(),
        "Should have has_manifest boolean"
    );
}

#[test]
fn subcommand_help_works() {
    agentjj()
        .args(["status", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Show repository status"));

    agentjj()
        .args(["orient", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("repository orientation"));

    agentjj()
        .args(["schema", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("JSON schemas"));

    agentjj()
        .args(["skill", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("skill documentation"));

    agentjj()
        .args(["quickstart", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("getting-started"));
}

// =============================================================================
// Skill command tests
// =============================================================================

#[test]
fn skill_returns_success() {
    agentjj()
        .arg("skill")
        .assert()
        .success()
        .stdout(predicate::str::contains("agentjj"));
}

#[test]
fn skill_json_returns_valid_json() {
    let output = agentjj().args(["--json", "skill"]).assert().success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Skill output should be valid JSON");

    assert!(json["content"].is_string(), "Should have content field");
    assert_eq!(
        json["format"].as_str(),
        Some("markdown"),
        "Should have format: markdown"
    );
}

#[test]
fn skill_works_outside_repo() {
    let tmp = TempDir::new().unwrap();

    agentjj()
        .arg("skill")
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("agentjj"));
}

// =============================================================================
// Quickstart command tests
// =============================================================================

#[test]
fn quickstart_returns_success() {
    agentjj()
        .arg("quickstart")
        .assert()
        .success()
        .stdout(predicate::str::contains("Quick Start"));
}

#[test]
fn quickstart_json_returns_valid_json() {
    let output = agentjj().args(["--json", "quickstart"]).assert().success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Quickstart output should be valid JSON");

    assert!(json["steps"].is_array(), "Should have steps array");
    assert!(json["tips"].is_array(), "Should have tips array");
    assert!(
        json["steps"].as_array().unwrap().len() == 6,
        "Should have exactly 6 steps"
    );
}

#[test]
fn quickstart_works_outside_repo() {
    let tmp = TempDir::new().unwrap();

    agentjj()
        .arg("quickstart")
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Quick Start"));
}

// =============================================================================
// Commit command tests
// =============================================================================

/// Helper to create a temp repo with an initial git commit so commit tests
/// have a parent to diff against. Also triggers jj auto-colocate.
fn setup_temp_repo_for_commit() -> Option<TempDir> {
    let tmp = TempDir::new().ok()?;

    // Initialize git repo
    let status = Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }

    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(tmp.path())
        .status()
        .ok()?;

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(tmp.path())
        .status()
        .ok()?;

    // Create initial file and commit so there's a parent
    std::fs::write(tmp.path().join("README.md"), "# Test Repository\n").ok()?;

    Command::new("git")
        .args(["add", "-A"])
        .current_dir(tmp.path())
        .status()
        .ok()?;

    Command::new("git")
        .args(["commit", "-m", "initial commit"])
        .current_dir(tmp.path())
        .status()
        .ok()?;

    // Run agentjj status to trigger auto-colocate
    let output = agentjj()
        .arg("status")
        .current_dir(tmp.path())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    Some(tmp)
}

#[test]
fn commit_with_changes_succeeds() {
    let Some(tmp) = setup_temp_repo_for_commit() else {
        eprintln!("Skipping test: could not set up temp repo");
        return;
    };

    // Create a new file to commit
    std::fs::write(tmp.path().join("hello.txt"), "hello world\n").unwrap();

    agentjj()
        .args(["commit", "-m", "feat: add hello file"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Committed"));
}

#[test]
fn commit_json_output() {
    let Some(tmp) = setup_temp_repo_for_commit() else {
        eprintln!("Skipping test: could not set up temp repo");
        return;
    };

    // Create a new file to commit
    std::fs::write(tmp.path().join("test.txt"), "test content\n").unwrap();

    let output = agentjj()
        .args(["--json", "commit", "-m", "feat: add test file"])
        .current_dir(tmp.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Commit output should be valid JSON");

    assert_eq!(json["committed"], true, "Should have committed: true");
    assert!(json["commit"].is_string(), "Should have commit hash");
    assert!(json["message"].is_string(), "Should have message");
    assert!(json["change_id"].is_string(), "Should have change_id");
    assert!(
        json["files_changed"].is_array(),
        "Should have files_changed array"
    );
}

#[test]
fn commit_nothing_to_commit() {
    let Some(tmp) = setup_temp_repo_for_commit() else {
        eprintln!("Skipping test: could not set up temp repo");
        return;
    };

    // Create .agent/.gitignore so TypedChange files are excluded from snapshots
    std::fs::create_dir_all(tmp.path().join(".agent")).unwrap();
    std::fs::write(
        tmp.path().join(".agent/.gitignore"),
        "changes/\ncheckpoints/\n",
    )
    .unwrap();

    // First commit syncs jj state (README.md + .agent/.gitignore are new to jj)
    agentjj()
        .args(["commit", "-m", "initial sync", "--no-invariants"])
        .current_dir(tmp.path())
        .assert()
        .success();

    // Now with no new changes, commit should fail
    agentjj()
        .args(["commit", "-m", "empty commit", "--no-invariants"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("nothing to commit"));
}

#[test]
fn commit_with_type_flag() {
    let Some(tmp) = setup_temp_repo_for_commit() else {
        eprintln!("Skipping test: could not set up temp repo");
        return;
    };

    // Create .agent dir for metadata storage
    std::fs::create_dir_all(tmp.path().join(".agent/changes")).ok();

    // Create a new file to commit
    std::fs::write(tmp.path().join("refactored.rs"), "fn main() {}\n").unwrap();

    agentjj()
        .args([
            "commit",
            "-m",
            "refactor: clean up code",
            "--type",
            "refactor",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    // Verify TypedChange metadata was saved
    let changes_dir = tmp.path().join(".agent/changes");
    assert!(
        changes_dir.exists(),
        "Changes directory should exist after commit"
    );

    // There should be at least one .toml file in the changes directory
    let change_files: Vec<_> = std::fs::read_dir(&changes_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "toml")
                .unwrap_or(false)
        })
        .collect();

    assert!(
        !change_files.is_empty(),
        "Should have at least one typed change file"
    );

    // Read and verify the change file
    let content = std::fs::read_to_string(change_files[0].path()).unwrap();
    assert!(
        content.contains("refactor"),
        "Change file should contain type 'refactor'"
    );
}

#[test]
fn commit_invariant_failure_blocks() {
    let Some(tmp) = setup_temp_repo_for_commit() else {
        eprintln!("Skipping test: could not set up temp repo");
        return;
    };

    // Set up a manifest with a failing invariant
    std::fs::create_dir_all(tmp.path().join(".agent")).ok();
    std::fs::write(
        tmp.path().join(".agent/manifest.toml"),
        r#"
[repo]
name = "test-repo"

[invariants]
always_fail = { cmd = "false", on = ["pre-commit"] }
"#,
    )
    .unwrap();

    // Create a new file to commit
    std::fs::write(tmp.path().join("should_not_commit.txt"), "blocked\n").unwrap();

    // Commit should fail due to invariant
    agentjj()
        .args(["commit", "-m", "should be blocked"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("invariant").or(predicate::str::contains("Invariant")));

    // Verify the file was NOT committed to git
    let log_output = Command::new("git")
        .args(["log", "--oneline", "-1"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let log_str = String::from_utf8_lossy(&log_output.stdout);
    assert!(
        !log_str.contains("should be blocked"),
        "Failed invariant should prevent commit from appearing in git log"
    );
}

// =============================================================================
// Graph command tests (richer output with timestamp, author, full_commit_id)
// =============================================================================

#[test]
fn graph_ascii_json_includes_rich_fields() {
    let Some(tmp) = setup_temp_repo_for_commit() else {
        eprintln!("Skipping test: could not set up temp repo");
        return;
    };

    // Create a file and commit so there's a node with author/timestamp data
    std::fs::write(tmp.path().join("feature.txt"), "new feature\n").unwrap();

    agentjj()
        .args(["commit", "-m", "feat: add feature file"])
        .current_dir(tmp.path())
        .assert()
        .success();

    // Now run graph ascii in JSON mode
    let output = agentjj()
        .args(["--json", "graph", "--format", "ascii"])
        .current_dir(tmp.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Graph ASCII JSON should be valid");

    assert_eq!(json["format"], "ascii", "Should report format as ascii");
    assert!(json["nodes"].is_array(), "Should have nodes array");

    let nodes = json["nodes"].as_array().unwrap();
    assert!(!nodes.is_empty(), "Should have at least one node");

    // Check that every node has the new rich fields
    for node in nodes {
        assert!(node["id"].is_string(), "Node should have id");
        assert!(node["parents"].is_array(), "Node should have parents");
        assert!(
            node["full_commit_id"].is_string(),
            "Node should have full_commit_id"
        );
        // full_commit_id should be longer than the truncated commit_id
        let full_id = node["full_commit_id"].as_str().unwrap();
        assert!(
            full_id.len() > 8,
            "full_commit_id should be the full hex hash, got len={}",
            full_id.len()
        );
    }
}

#[test]
fn graph_mermaid_json_includes_rich_fields() {
    let Some(tmp) = setup_temp_repo_for_commit() else {
        eprintln!("Skipping test: could not set up temp repo");
        return;
    };

    std::fs::write(tmp.path().join("mermaid.txt"), "mermaid test\n").unwrap();

    agentjj()
        .args(["commit", "-m", "feat: mermaid test"])
        .current_dir(tmp.path())
        .assert()
        .success();

    let output = agentjj()
        .args(["--json", "graph", "--format", "mermaid"])
        .current_dir(tmp.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Graph mermaid JSON should be valid");

    assert_eq!(json["format"], "mermaid");
    let nodes = json["nodes"].as_array().unwrap();
    assert!(!nodes.is_empty());

    for node in nodes {
        assert!(
            node["full_commit_id"].is_string(),
            "Should have full_commit_id"
        );
        // timestamp and author may be null for root/empty commits, but the field must exist
        assert!(
            node.get("timestamp").is_some(),
            "Node should have timestamp field"
        );
        assert!(
            node.get("author").is_some(),
            "Node should have author field"
        );
    }
}

#[test]
fn graph_dot_json_includes_rich_fields() {
    let Some(tmp) = setup_temp_repo_for_commit() else {
        eprintln!("Skipping test: could not set up temp repo");
        return;
    };

    std::fs::write(tmp.path().join("dot.txt"), "dot test\n").unwrap();

    agentjj()
        .args(["commit", "-m", "feat: dot test"])
        .current_dir(tmp.path())
        .assert()
        .success();

    let output = agentjj()
        .args(["--json", "graph", "--format", "dot"])
        .current_dir(tmp.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Graph DOT JSON should be valid");

    assert_eq!(json["format"], "dot");
    let nodes = json["nodes"].as_array().unwrap();
    assert!(!nodes.is_empty());

    for node in nodes {
        assert!(
            node["full_commit_id"].is_string(),
            "Should have full_commit_id"
        );
        assert!(
            node.get("timestamp").is_some(),
            "Node should have timestamp field"
        );
        assert!(
            node.get("author").is_some(),
            "Node should have author field"
        );
    }
}

// =============================================================================
// Commit --paths tests (selective commit)
// =============================================================================

#[test]
fn commit_paths_includes_only_specified_files() {
    let Some(tmp) = setup_temp_repo_for_commit() else {
        eprintln!("Skipping test: could not set up temp repo");
        return;
    };

    // Create two files
    std::fs::write(tmp.path().join("included.txt"), "I should be committed\n").unwrap();
    std::fs::write(
        tmp.path().join("excluded.txt"),
        "I should NOT be committed\n",
    )
    .unwrap();

    // Commit only included.txt using --paths
    let output = agentjj()
        .args([
            "--json",
            "commit",
            "-m",
            "feat: add included file",
            "--no-invariants",
            "--paths",
            "included.txt",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Commit output should be valid JSON");

    assert_eq!(json["committed"], true);
    let files = json["files_changed"].as_array().unwrap();

    // Only the specified path should appear in files_changed
    let file_names: Vec<&str> = files.iter().map(|f| f.as_str().unwrap()).collect();
    assert!(
        file_names.contains(&"included.txt"),
        "files_changed should contain included.txt, got {:?}",
        file_names
    );
    assert!(
        !file_names.contains(&"excluded.txt"),
        "files_changed should NOT contain excluded.txt, got {:?}",
        file_names
    );
}

#[test]
fn commit_paths_leaves_other_changes_in_working_copy() {
    let Some(tmp) = setup_temp_repo_for_commit() else {
        eprintln!("Skipping test: could not set up temp repo");
        return;
    };

    // Create two files
    std::fs::write(tmp.path().join("committed.txt"), "committed content\n").unwrap();
    std::fs::write(tmp.path().join("remaining.txt"), "remaining content\n").unwrap();

    // Commit only committed.txt
    agentjj()
        .args([
            "commit",
            "-m",
            "feat: add committed file",
            "--no-invariants",
            "--paths",
            "committed.txt",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    // The remaining file should still exist on disk (not lost)
    assert!(
        tmp.path().join("remaining.txt").exists(),
        "remaining.txt should still exist on filesystem after selective commit"
    );
    let content = std::fs::read_to_string(tmp.path().join("remaining.txt")).unwrap();
    assert_eq!(
        content, "remaining content\n",
        "remaining.txt content should be unchanged"
    );

    // A subsequent commit (which snapshots the filesystem) should pick up
    // the remaining file, proving it was left in the working copy.
    let output = agentjj()
        .args([
            "--json",
            "commit",
            "-m",
            "feat: add remaining file",
            "--no-invariants",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Commit output should be valid JSON");

    let files = json["files_changed"].as_array().unwrap();
    let file_names: Vec<&str> = files.iter().map(|f| f.as_str().unwrap()).collect();
    assert!(
        file_names.contains(&"remaining.txt"),
        "remaining.txt should be picked up by the next commit, got {:?}",
        file_names
    );
}

#[test]
fn commit_paths_nonexistent_path_errors() {
    let Some(tmp) = setup_temp_repo_for_commit() else {
        eprintln!("Skipping test: could not set up temp repo");
        return;
    };

    // Create a file so there's something to commit
    std::fs::write(tmp.path().join("real.txt"), "real content\n").unwrap();

    // Try to commit a path that doesn't exist
    agentjj()
        .args([
            "commit",
            "-m",
            "should fail",
            "--no-invariants",
            "--paths",
            "nonexistent.txt",
        ])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
}

#[test]
fn commit_paths_unchanged_path_no_changes_error() {
    let Some(tmp) = setup_temp_repo_for_commit() else {
        eprintln!("Skipping test: could not set up temp repo");
        return;
    };

    // Create .agent/.gitignore so TypedChange files don't pollute snapshots
    std::fs::create_dir_all(tmp.path().join(".agent")).unwrap();
    std::fs::write(
        tmp.path().join(".agent/.gitignore"),
        "changes/\ncheckpoints/\n",
    )
    .unwrap();

    // Create a file and commit it first
    std::fs::write(tmp.path().join("stable.txt"), "stable content\n").unwrap();
    agentjj()
        .args([
            "commit",
            "-m",
            "initial: add stable file",
            "--no-invariants",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    // Now create a new file but try to commit only the unchanged stable.txt
    std::fs::write(tmp.path().join("new.txt"), "new content\n").unwrap();

    agentjj()
        .args([
            "commit",
            "-m",
            "should fail",
            "--no-invariants",
            "--paths",
            "stable.txt",
        ])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("no changes in specified paths"));
}

#[test]
fn commit_paths_multiple_paths() {
    let Some(tmp) = setup_temp_repo_for_commit() else {
        eprintln!("Skipping test: could not set up temp repo");
        return;
    };

    // Create three files
    std::fs::write(tmp.path().join("a.txt"), "file a\n").unwrap();
    std::fs::write(tmp.path().join("b.txt"), "file b\n").unwrap();
    std::fs::write(tmp.path().join("c.txt"), "file c\n").unwrap();

    // Commit only a.txt and b.txt
    let output = agentjj()
        .args([
            "--json",
            "commit",
            "-m",
            "feat: add a and b",
            "--no-invariants",
            "--paths",
            "a.txt",
            "b.txt",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Commit output should be valid JSON");

    let files = json["files_changed"].as_array().unwrap();
    let file_names: Vec<&str> = files.iter().map(|f| f.as_str().unwrap()).collect();

    assert!(file_names.contains(&"a.txt"), "Should include a.txt");
    assert!(file_names.contains(&"b.txt"), "Should include b.txt");
    assert!(!file_names.contains(&"c.txt"), "Should NOT include c.txt");
}

#[test]
fn commit_without_paths_unchanged_behavior() {
    let Some(tmp) = setup_temp_repo_for_commit() else {
        eprintln!("Skipping test: could not set up temp repo");
        return;
    };

    // Create two files and commit without --paths (should include both)
    std::fs::write(tmp.path().join("one.txt"), "file one\n").unwrap();
    std::fs::write(tmp.path().join("two.txt"), "file two\n").unwrap();

    let output = agentjj()
        .args([
            "--json",
            "commit",
            "-m",
            "feat: add both files",
            "--no-invariants",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Commit output should be valid JSON");

    let files = json["files_changed"].as_array().unwrap();
    let file_names: Vec<&str> = files.iter().map(|f| f.as_str().unwrap()).collect();

    assert!(
        file_names.contains(&"one.txt"),
        "Should include one.txt when --paths not used"
    );
    assert!(
        file_names.contains(&"two.txt"),
        "Should include two.txt when --paths not used"
    );
}

#[test]
fn commit_on_feature_branch_does_not_move_main() {
    let Some(tmp) = setup_temp_repo_for_commit() else {
        eprintln!("Skipping test: could not set up temp repo");
        return;
    };

    // Create and switch to a feature branch
    Command::new("git")
        .args(["checkout", "-b", "feature-xyz"])
        .current_dir(tmp.path())
        .status()
        .unwrap();

    // Record where main points before our commit
    let main_before = Command::new("git")
        .args(["rev-parse", "main"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let main_before_sha = String::from_utf8_lossy(&main_before.stdout)
        .trim()
        .to_string();

    // Make a change on the feature branch
    std::fs::write(tmp.path().join("feature.txt"), "feature work\n").unwrap();

    agentjj()
        .args(["commit", "-m", "feat: feature branch work"])
        .current_dir(tmp.path())
        .assert()
        .success();

    // Verify main was NOT moved
    let main_after = Command::new("git")
        .args(["rev-parse", "main"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let main_after_sha = String::from_utf8_lossy(&main_after.stdout)
        .trim()
        .to_string();

    assert_eq!(
        main_before_sha, main_after_sha,
        "Committing on feature branch should not move main"
    );
}

#[test]
fn commit_on_detached_head_warns_about_git_sync() {
    let Some(tmp) = setup_temp_repo_for_commit() else {
        eprintln!("Skipping test: could not set up temp repo");
        return;
    };

    // Detach HEAD
    Command::new("git")
        .args(["checkout", "--detach", "HEAD"])
        .current_dir(tmp.path())
        .status()
        .unwrap();

    // Make a change
    std::fs::write(tmp.path().join("detached.txt"), "detached work\n").unwrap();

    // Commit should succeed but warn about detached HEAD
    let output = agentjj()
        .args(["commit", "-m", "feat: detached head commit"])
        .current_dir(tmp.path())
        .assert()
        .success();

    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    assert!(
        stderr.contains("HEAD is detached"),
        "Should warn about detached HEAD skipping git branch sync, got: {}",
        stderr
    );
}

// =============================================================================
// Test: first agentjj commit inherits git history (not disconnected)
// =============================================================================

#[test]
fn first_commit_has_git_head_as_ancestor() {
    let tmp = TempDir::new().unwrap();

    // Initialize a git repository with a commit
    Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .status()
        .unwrap();

    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(tmp.path())
        .status()
        .unwrap();

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(tmp.path())
        .status()
        .unwrap();

    std::fs::write(tmp.path().join("README.md"), "# Test\n").unwrap();

    Command::new("git")
        .args(["add", "-A"])
        .current_dir(tmp.path())
        .status()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "initial git commit"])
        .current_dir(tmp.path())
        .status()
        .unwrap();

    // Get git HEAD hash before agentjj touches anything
    let git_head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let git_head_hex = String::from_utf8_lossy(&git_head.stdout).trim().to_string();
    assert!(!git_head_hex.is_empty(), "git HEAD should exist");

    // Make a change and commit via agentjj (triggers auto-colocate init)
    std::fs::write(tmp.path().join("new.txt"), "new content\n").unwrap();

    agentjj()
        .args(["commit", "-m", "feat: first agentjj commit"])
        .current_dir(tmp.path())
        .assert()
        .success();

    // Verify: git log should show BOTH commits (agentjj's and the original)
    // If init_colocated_git doesn't import refs, the agentjj commit would be
    // disconnected with no parent, and "initial git commit" would not appear
    // in the log starting from HEAD.
    let log_output = Command::new("git")
        .args(["log", "--oneline", "--all"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let log_text = String::from_utf8_lossy(&log_output.stdout);

    assert!(
        log_text.contains("initial git commit"),
        "git log should contain the original git commit, but got:\n{}",
        log_text
    );
    assert!(
        log_text.contains("first agentjj commit"),
        "git log should contain the agentjj commit, but got:\n{}",
        log_text
    );

    // Verify the agentjj commit is an ancestor of git HEAD (connected history)
    let ancestor_check = Command::new("git")
        .args(["log", "--oneline", "HEAD"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let ancestor_text = String::from_utf8_lossy(&ancestor_check.stdout);

    // Both commits should be reachable from HEAD (connected lineage)
    assert!(
        ancestor_text.contains("initial git commit"),
        "Original git commit should be reachable from HEAD (connected history), got:\n{}",
        ancestor_text
    );
}
