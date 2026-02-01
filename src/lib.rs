// ABOUTME: Library root for agentjj - agent-oriented jj porcelain
// ABOUTME: Exports manifest, typed changes, intent transactions, and repo operations

pub mod manifest;
pub mod change;
pub mod intent;
pub mod repo;
pub mod error;
pub mod symbols;

pub use error::{Error, Result};
pub use manifest::Manifest;
pub use change::{TypedChange, ChangeType, ChangeCategory};
pub use intent::{Intent, IntentResult};
pub use symbols::{Symbol, SymbolKind, SymbolContext, SupportedLanguage};
