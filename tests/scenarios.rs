// ABOUTME: Scenario-based integration tests for agentjj CLI
// ABOUTME: Simulates real agent workflows using assert_cmd, predicates, and tempfile

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::process::Command as StdCommand;
use tempfile::TempDir;

/// Helper to create a jj repository in a temp directory
fn setup_jj_repo() -> TempDir {
    let tmp = TempDir::new().expect("Failed to create temp directory");

    // Initialize jj git repo
    let output = StdCommand::new("jj")
        .args(["git", "init"])
        .current_dir(tmp.path())
        .output()
        .expect("Failed to run jj git init");

    assert!(
        output.status.success(),
        "jj git init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    tmp
}

/// Helper to get our binary command
fn agentjj() -> Command {
    Command::cargo_bin("agentjj").expect("Failed to find agentjj binary")
}

// =============================================================================
// Scenario 1: New Agent Workflow
// =============================================================================

mod new_agent_workflow {
    use super::*;

    #[test]
    fn init_creates_manifest() {
        let tmp = setup_jj_repo();

        // Run agentjj init
        agentjj()
            .current_dir(tmp.path())
            .args(["init", "--name", "test-project"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Initialized agentjj"));

        // Verify .agent/manifest.toml exists
        let manifest_path = tmp.path().join(".agent/manifest.toml");
        assert!(manifest_path.exists(), "Manifest file should be created");

        // Verify manifest content
        let content = fs::read_to_string(&manifest_path).expect("Failed to read manifest");
        assert!(
            content.contains("test-project"),
            "Manifest should contain project name"
        );
    }

    #[test]
    fn init_json_output() {
        let tmp = setup_jj_repo();

        // Run agentjj init with JSON output
        agentjj()
            .current_dir(tmp.path())
            .args(["--json", "init", "--name", "json-test"])
            .assert()
            .success()
            .stdout(predicate::str::contains(r#""status": "created""#))
            .stdout(predicate::str::contains(r#""name": "json-test""#));
    }

    #[test]
    fn init_already_exists() {
        let tmp = setup_jj_repo();

        // Initialize once
        agentjj()
            .current_dir(tmp.path())
            .args(["init", "--name", "first"])
            .assert()
            .success();

        // Initialize again - should report exists
        agentjj()
            .current_dir(tmp.path())
            .args(["init", "--name", "second"])
            .assert()
            .success()
            .stdout(predicate::str::contains("already exists"));
    }

    #[test]
    fn orient_returns_complete_orientation() {
        let tmp = setup_jj_repo();

        // First initialize
        agentjj()
            .current_dir(tmp.path())
            .args(["init", "--name", "orient-test"])
            .assert()
            .success();

        // Run orient with JSON output
        let output = agentjj()
            .current_dir(tmp.path())
            .args(["--json", "orient"])
            .assert()
            .success();

        // Parse and verify JSON structure
        let stdout = String::from_utf8_lossy(&output.get_output().stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("Output should be valid JSON");

        // Verify required fields exist
        assert!(json.get("current_state").is_some(), "Should have current_state");
        assert!(
            json["current_state"].get("change_id").is_some(),
            "Should have change_id"
        );
        assert!(json.get("capabilities").is_some(), "Should have capabilities");
        assert!(json.get("quick_start").is_some(), "Should have quick_start");
        assert!(json.get("codebase").is_some(), "Should have codebase info");
    }
}

// =============================================================================
// Scenario 2: Checkpoint and Recovery
// =============================================================================

mod checkpoint_and_recovery {
    use super::*;

    #[test]
    fn create_checkpoint() {
        let tmp = setup_jj_repo();

        // Initialize agentjj
        agentjj()
            .current_dir(tmp.path())
            .args(["init"])
            .assert()
            .success();

        // Create a checkpoint
        agentjj()
            .current_dir(tmp.path())
            .args(["checkpoint", "test-recovery", "-d", "Test checkpoint"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Checkpoint 'test-recovery' created"));

        // Verify checkpoint file exists
        let checkpoint_path = tmp.path().join(".agent/checkpoints/test-recovery.json");
        assert!(
            checkpoint_path.exists(),
            "Checkpoint file should exist at {:?}",
            checkpoint_path
        );

        // Verify checkpoint content
        let content = fs::read_to_string(&checkpoint_path).expect("Failed to read checkpoint");
        let json: serde_json::Value =
            serde_json::from_str(&content).expect("Checkpoint should be valid JSON");
        assert_eq!(json["name"], "test-recovery");
        assert!(json.get("change_id").is_some());
        assert!(json.get("operation_id").is_some());
    }

    #[test]
    fn checkpoint_json_output() {
        let tmp = setup_jj_repo();

        agentjj()
            .current_dir(tmp.path())
            .args(["init"])
            .assert()
            .success();

        agentjj()
            .current_dir(tmp.path())
            .args(["--json", "checkpoint", "json-check"])
            .assert()
            .success()
            .stdout(predicate::str::contains(r#""created": true"#))
            .stdout(predicate::str::contains("restore_command"));
    }

    #[test]
    fn undo_dry_run_to_checkpoint() {
        let tmp = setup_jj_repo();

        // Initialize and create checkpoint
        agentjj()
            .current_dir(tmp.path())
            .args(["init"])
            .assert()
            .success();

        agentjj()
            .current_dir(tmp.path())
            .args(["checkpoint", "recovery-point"])
            .assert()
            .success();

        // Make a change (create a new file via jj)
        fs::write(tmp.path().join("newfile.txt"), "test content").expect("Failed to create file");

        // Dry run undo to checkpoint
        agentjj()
            .current_dir(tmp.path())
            .args(["undo", "--to", "recovery-point", "--dry-run"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Would restore to checkpoint"));
    }

    #[test]
    fn undo_dry_run_json() {
        let tmp = setup_jj_repo();

        agentjj()
            .current_dir(tmp.path())
            .args(["init"])
            .assert()
            .success();

        agentjj()
            .current_dir(tmp.path())
            .args(["checkpoint", "json-recovery"])
            .assert()
            .success();

        let output = agentjj()
            .current_dir(tmp.path())
            .args(["--json", "undo", "--to", "json-recovery", "--dry-run"])
            .assert()
            .success();

        let stdout = String::from_utf8_lossy(&output.get_output().stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("Output should be valid JSON");

        assert_eq!(json["dry_run"], true);
        assert_eq!(json["checkpoint"], "json-recovery");
        assert!(json.get("would_restore_to").is_some());
    }
}

// =============================================================================
// Scenario 3: Typed Change Workflow
// =============================================================================

mod typed_change_workflow {
    use super::*;

    #[test]
    fn set_typed_change() {
        let tmp = setup_jj_repo();

        // Initialize
        agentjj()
            .current_dir(tmp.path())
            .args(["init"])
            .assert()
            .success();

        // Set a typed change
        agentjj()
            .current_dir(tmp.path())
            .args([
                "change",
                "set",
                "-i",
                "Test feature implementation",
                "-t",
                "behavioral",
                "-c",
                "feature",
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("Saved typed change"));

        // Verify change file was created
        let changes_dir = tmp.path().join(".agent/changes");
        assert!(changes_dir.exists(), "Changes directory should exist");

        // Should have at least one .toml file
        let toml_files: Vec<_> = fs::read_dir(&changes_dir)
            .expect("Failed to read changes dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|ext| ext == "toml").unwrap_or(false))
            .collect();

        assert!(!toml_files.is_empty(), "Should have at least one change file");
    }

    #[test]
    fn list_typed_changes() {
        let tmp = setup_jj_repo();

        agentjj()
            .current_dir(tmp.path())
            .args(["init"])
            .assert()
            .success();

        // Set a change first
        agentjj()
            .current_dir(tmp.path())
            .args(["change", "set", "-i", "First change", "-t", "behavioral"])
            .assert()
            .success();

        // List changes
        agentjj()
            .current_dir(tmp.path())
            .args(["change", "list"])
            .assert()
            .success()
            .stdout(predicate::str::contains("First change"));
    }

    #[test]
    fn status_shows_typed_change() {
        let tmp = setup_jj_repo();

        agentjj()
            .current_dir(tmp.path())
            .args(["init"])
            .assert()
            .success();

        // Set a change
        agentjj()
            .current_dir(tmp.path())
            .args(["change", "set", "-i", "Status test", "-t", "refactor"])
            .assert()
            .success();

        // Check status with JSON
        let output = agentjj()
            .current_dir(tmp.path())
            .args(["--json", "status"])
            .assert()
            .success();

        let stdout = String::from_utf8_lossy(&output.get_output().stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("Output should be valid JSON");

        // Verify typed_change is present
        assert!(
            json.get("typed_change").is_some(),
            "Status should include typed_change"
        );

        // The typed_change should not be null since we just set one
        if let Some(tc) = json.get("typed_change") {
            if !tc.is_null() {
                assert!(
                    tc.get("intent").is_some() || tc.get("type").is_some(),
                    "typed_change should have intent or type"
                );
            }
        }
    }
}

// =============================================================================
// Scenario 4: Error Handling
// =============================================================================

mod error_handling {
    use super::*;

    #[test]
    fn error_in_non_repo_directory() {
        let tmp = TempDir::new().expect("Failed to create temp directory");

        // Run status in non-repo directory
        agentjj()
            .current_dir(tmp.path())
            .args(["status"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("No git or jj repository found"));
    }

    #[test]
    fn error_json_format() {
        let tmp = TempDir::new().expect("Failed to create temp directory");

        // Run with --json in non-repo directory
        let output = agentjj()
            .current_dir(tmp.path())
            .args(["--json", "status"])
            .assert()
            .failure();

        let stdout = String::from_utf8_lossy(&output.get_output().stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("Error output should be valid JSON");

        assert_eq!(json["error"], true, "Should have error: true");
        assert!(json.get("message").is_some(), "Should have error message");
    }

    #[test]
    fn change_show_nonexistent() {
        let tmp = setup_jj_repo();

        agentjj()
            .current_dir(tmp.path())
            .args(["init"])
            .assert()
            .success();

        // Try to show a nonexistent change
        agentjj()
            .current_dir(tmp.path())
            .args(["change", "show", "nonexistent123"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("not found").or(predicate::str::contains("Change")));
    }

    #[test]
    fn undo_to_nonexistent_checkpoint() {
        let tmp = setup_jj_repo();

        agentjj()
            .current_dir(tmp.path())
            .args(["init"])
            .assert()
            .success();

        // Try to undo to nonexistent checkpoint
        agentjj()
            .current_dir(tmp.path())
            .args(["undo", "--to", "nonexistent"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("not found"));
    }

    #[test]
    fn init_without_jj_repo() {
        let tmp = TempDir::new().expect("Failed to create temp directory");

        // Try to init without jj repo
        agentjj()
            .current_dir(tmp.path())
            .args(["init"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("No git or jj repository found"));
    }
}

// =============================================================================
// Scenario 5: Bulk Operations
// =============================================================================

mod bulk_operations {
    use super::*;

    #[test]
    fn bulk_read_multiple_files() {
        let tmp = setup_jj_repo();

        // Create some test files
        fs::write(tmp.path().join("file1.txt"), "Content of file 1").expect("Failed to write file1");
        fs::write(tmp.path().join("file2.txt"), "Content of file 2").expect("Failed to write file2");

        agentjj()
            .current_dir(tmp.path())
            .args(["init"])
            .assert()
            .success();

        // Bulk read both files
        agentjj()
            .current_dir(tmp.path())
            .args(["bulk", "read", "file1.txt", "file2.txt"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Content of file 1"))
            .stdout(predicate::str::contains("Content of file 2"));
    }

    #[test]
    fn bulk_read_json_output() {
        let tmp = setup_jj_repo();

        fs::write(tmp.path().join("a.txt"), "File A").expect("Failed to write");
        fs::write(tmp.path().join("b.txt"), "File B").expect("Failed to write");

        agentjj()
            .current_dir(tmp.path())
            .args(["init"])
            .assert()
            .success();

        let output = agentjj()
            .current_dir(tmp.path())
            .args(["--json", "bulk", "read", "a.txt", "b.txt"])
            .assert()
            .success();

        let stdout = String::from_utf8_lossy(&output.get_output().stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("Output should be valid JSON");

        assert!(json.get("files").is_some(), "Should have files array");
        assert!(json.get("summary").is_some(), "Should have summary");

        let files = json["files"].as_array().expect("files should be an array");
        assert_eq!(files.len(), 2, "Should have read 2 files");
    }

    #[test]
    fn bulk_read_with_errors() {
        let tmp = setup_jj_repo();

        fs::write(tmp.path().join("exists.txt"), "I exist").expect("Failed to write");

        agentjj()
            .current_dir(tmp.path())
            .args(["init"])
            .assert()
            .success();

        let output = agentjj()
            .current_dir(tmp.path())
            .args(["--json", "bulk", "read", "exists.txt", "nonexistent.txt"])
            .assert()
            .success(); // Should still succeed overall

        let stdout = String::from_utf8_lossy(&output.get_output().stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("Output should be valid JSON");

        assert!(json.get("errors").is_some(), "Should report errors");
        let errors = json["errors"].as_array().expect("errors should be an array");
        assert!(!errors.is_empty(), "Should have at least one error");
    }

    #[test]
    fn files_pattern_filtering() {
        let tmp = setup_jj_repo();

        // Create files with different extensions
        fs::create_dir_all(tmp.path().join("src")).expect("Failed to create src dir");
        fs::write(tmp.path().join("src/main.rs"), "fn main() {}").expect("Failed to write");
        fs::write(tmp.path().join("src/lib.rs"), "pub mod test;").expect("Failed to write");
        fs::write(tmp.path().join("README.md"), "# Readme").expect("Failed to write");

        agentjj()
            .current_dir(tmp.path())
            .args(["init"])
            .assert()
            .success();

        // Filter for .rs files
        let output = agentjj()
            .current_dir(tmp.path())
            .args(["--json", "files", "--pattern", "src/*.rs"])
            .assert()
            .success();

        let stdout = String::from_utf8_lossy(&output.get_output().stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("Output should be valid JSON");

        let files = json["files"].as_array().expect("files should be an array");

        // All returned files should be .rs
        for file in files {
            let path = file["path"].as_str().expect("path should be string");
            assert!(path.ends_with(".rs"), "File {} should be a .rs file", path);
        }
    }

    #[test]
    fn files_shows_count() {
        let tmp = setup_jj_repo();

        fs::write(tmp.path().join("one.txt"), "1").expect("Failed to write");
        fs::write(tmp.path().join("two.txt"), "2").expect("Failed to write");
        fs::write(tmp.path().join("three.txt"), "3").expect("Failed to write");

        agentjj()
            .current_dir(tmp.path())
            .args(["init"])
            .assert()
            .success();

        let output = agentjj()
            .current_dir(tmp.path())
            .args(["--json", "files", "--pattern", "*.txt"])
            .assert()
            .success();

        let stdout = String::from_utf8_lossy(&output.get_output().stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("Output should be valid JSON");

        let count = json["count"].as_u64().expect("count should be a number");
        assert_eq!(count, 3, "Should find 3 .txt files");
    }
}

// =============================================================================
// Additional Workflow Tests
// =============================================================================

mod additional_workflows {
    use super::*;

    #[test]
    fn status_basic() {
        let tmp = setup_jj_repo();

        agentjj()
            .current_dir(tmp.path())
            .args(["status"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Change:"));
    }

    #[test]
    fn status_json() {
        let tmp = setup_jj_repo();

        let output = agentjj()
            .current_dir(tmp.path())
            .args(["--json", "status"])
            .assert()
            .success();

        let stdout = String::from_utf8_lossy(&output.get_output().stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("Output should be valid JSON");

        assert!(json.get("change_id").is_some());
        assert!(json.get("operation_id").is_some());
        assert!(json.get("has_manifest").is_some());
    }

    #[test]
    fn manifest_show() {
        let tmp = setup_jj_repo();

        agentjj()
            .current_dir(tmp.path())
            .args(["init", "--name", "manifest-test"])
            .assert()
            .success();

        agentjj()
            .current_dir(tmp.path())
            .args(["manifest", "show"])
            .assert()
            .success()
            .stdout(predicate::str::contains("manifest-test"));
    }

    #[test]
    fn manifest_validate() {
        let tmp = setup_jj_repo();

        agentjj()
            .current_dir(tmp.path())
            .args(["init", "--name", "validate-test"])
            .assert()
            .success();

        agentjj()
            .current_dir(tmp.path())
            .args(["manifest", "validate"])
            .assert()
            .success()
            .stdout(predicate::str::contains("valid"));
    }

    #[test]
    fn schema_list() {
        let tmp = setup_jj_repo();

        agentjj()
            .current_dir(tmp.path())
            .args(["init"])
            .assert()
            .success();

        agentjj()
            .current_dir(tmp.path())
            .args(["schema"])
            .assert()
            .success()
            .stdout(predicate::str::contains("status"))
            .stdout(predicate::str::contains("symbol"));
    }

    #[test]
    fn suggest_actions() {
        let tmp = setup_jj_repo();

        // Without manifest, should suggest init
        agentjj()
            .current_dir(tmp.path())
            .args(["suggest"])
            .assert()
            .success()
            .stdout(predicate::str::contains("init"));
    }
}
