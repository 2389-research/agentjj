// ABOUTME: Error types for agentjj operations
// ABOUTME: Provides structured errors that agents can parse and act on

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Error {
    #[error("manifest not found at {path}")]
    ManifestNotFound { path: String },

    #[error("manifest parse error: {message}")]
    ManifestParse {
        message: String,
        line: Option<usize>,
    },

    #[error("precondition failed: {reason}")]
    PreconditionFailed {
        reason: String,
        expected: String,
        actual: String,
    },

    #[error("conflict in {file_count} files")]
    Conflict {
        file_count: usize,
        conflicts: Vec<ConflictDetail>,
        operation_id: String,
    },

    #[error("invariant '{name}' failed (command: `{command}`, exit code: {exit_code})")]
    InvariantFailed {
        name: String,
        command: String,
        exit_code: i32,
        stdout: String,
        stderr: String,
    },

    #[error("permission denied: {action} on {path}")]
    PermissionDenied { action: String, path: String },

    #[error("change {change_id} not found")]
    ChangeNotFound { change_id: String },

    #[error("repository error: {message}")]
    Repository { message: String },

    #[error("io error: {message}")]
    Io { message: String },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConflictDetail {
    pub file: String,
    pub ours: String,
    pub theirs: String,
    pub base: Option<String>,
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io {
            message: e.to_string(),
        }
    }
}

impl From<toml::de::Error> for Error {
    fn from(e: toml::de::Error) -> Self {
        Error::ManifestParse {
            message: e.message().to_string(),
            line: e.span().map(|s| s.start),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_serialize_to_json() {
        let err = Error::PreconditionFailed {
            reason: "main has advanced".into(),
            expected: "abc123".into(),
            actual: "def456".into(),
        };

        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("precondition_failed"));
        assert!(json.contains("main has advanced"));
    }

    #[test]
    fn conflict_error_includes_details() {
        let err = Error::Conflict {
            file_count: 2,
            conflicts: vec![ConflictDetail {
                file: "src/api.py".into(),
                ours: "fn a()".into(),
                theirs: "fn b()".into(),
                base: Some("fn original()".into()),
            }],
            operation_id: "op123".into(),
        };

        let json = serde_json::to_string_pretty(&err).unwrap();
        assert!(json.contains("src/api.py"));
        assert!(json.contains("op123"));
    }
}
