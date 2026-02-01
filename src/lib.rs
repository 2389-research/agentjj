// ABOUTME: Library root for agentjj - agent-oriented jj porcelain
// ABOUTME: Exports manifest, typed changes, intent transactions, and repo operations

pub mod change;
pub mod error;
pub mod intent;
pub mod manifest;
pub mod repo;
pub mod symbols;

pub use change::{ChangeCategory, ChangeType, TypedChange};
pub use error::{Error, Result};
pub use intent::{Intent, IntentResult};
pub use manifest::Manifest;
pub use symbols::{SupportedLanguage, Symbol, SymbolContext, SymbolKind};
