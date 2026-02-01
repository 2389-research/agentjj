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
        .stdout(predicate::str::contains("schema"));
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
}
