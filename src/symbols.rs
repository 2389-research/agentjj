// ABOUTME: Symbol extraction and querying using tree-sitter
// ABOUTME: Provides function signatures, class definitions, and minimal context for agents

use serde::{Deserialize, Serialize};
use std::path::Path;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};

use crate::error::{Error, Result};

/// A symbol extracted from source code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub signature: Option<String>,
    pub docstring: Option<String>,
    pub start_line: usize,
    pub end_line: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Symbol>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Constant,
    Variable,
    Module,
    Import,
}

/// Supported languages for symbol extraction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedLanguage {
    Python,
    Rust,
    JavaScript,
    TypeScript,
}

impl SupportedLanguage {
    /// Detect language from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "py" => Some(Self::Python),
            "rs" => Some(Self::Rust),
            "js" | "jsx" | "mjs" => Some(Self::JavaScript),
            "ts" | "tsx" => Some(Self::TypeScript),
            _ => None,
        }
    }

    /// Detect language from file path
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(Self::from_extension)
    }

    /// Get the tree-sitter language
    fn tree_sitter_language(&self) -> Language {
        match self {
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        }
    }

    /// Get the query for extracting symbols
    fn symbol_query(&self) -> &'static str {
        match self {
            Self::Python => PYTHON_SYMBOL_QUERY,
            Self::Rust => RUST_SYMBOL_QUERY,
            Self::JavaScript | Self::TypeScript => JS_SYMBOL_QUERY,
        }
    }
}

// Tree-sitter queries for different languages
const PYTHON_SYMBOL_QUERY: &str = r#"
(function_definition
  name: (identifier) @function.name
  parameters: (parameters) @function.params
  return_type: (type)? @function.return_type
  body: (block
    (expression_statement
      (string) @function.docstring)?)?
) @function.def

(class_definition
  name: (identifier) @class.name
  body: (block
    (expression_statement
      (string) @class.docstring)?)?
) @class.def

(decorated_definition
  (decorator)* @decorator
  definition: (_) @decorated.def
)
"#;

const RUST_SYMBOL_QUERY: &str = r#"
(function_item
  name: (identifier) @function.name
  parameters: (parameters) @function.params
  return_type: (type_identifier)? @function.return_type
) @function.def

(struct_item
  name: (type_identifier) @struct.name
) @struct.def

(enum_item
  name: (type_identifier) @enum.name
) @enum.def

(impl_item
  trait: (type_identifier)? @impl.trait
  type: (type_identifier) @impl.type
) @impl.def

(trait_item
  name: (type_identifier) @trait.name
) @trait.def
"#;

const JS_SYMBOL_QUERY: &str = r#"
(function_declaration
  name: (identifier) @function.name
  parameters: (formal_parameters) @function.params
) @function.def

(class_declaration
  name: (identifier) @class.name
  body: (class_body) @class.body
) @class.def

(method_definition
  name: (property_identifier) @method.name
  parameters: (formal_parameters) @method.params
) @method.def

(arrow_function
  parameters: (_) @arrow.params
  body: (_) @arrow.body
) @arrow.def

(lexical_declaration
  (variable_declarator
    name: (identifier) @const.name
    value: (_) @const.value
  )
) @const.def
"#;

/// Extract symbols from source code
pub fn extract_symbols(source: &str, language: SupportedLanguage) -> Result<Vec<Symbol>> {
    let mut parser = Parser::new();
    parser
        .set_language(&language.tree_sitter_language())
        .map_err(|e| Error::Repository {
            message: format!("Failed to set language: {}", e),
        })?;

    let tree = parser.parse(source, None).ok_or_else(|| Error::Repository {
        message: "Failed to parse source".into(),
    })?;

    let query = Query::new(&language.tree_sitter_language(), language.symbol_query())
        .map_err(|e| Error::Repository {
            message: format!("Failed to compile query: {}", e),
        })?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

    let mut symbols = Vec::new();
    let source_bytes = source.as_bytes();

    // StreamingIterator pattern: advance then get
    while let Some(m) = {
        matches.advance();
        matches.get()
    } {
        let mut name = None;
        let mut kind = SymbolKind::Function;
        let mut signature = None;
        let mut docstring = None;
        let mut start_line = 0;
        let mut end_line = 0;

        for capture in m.captures {
            let capture_name: &str = &query.capture_names()[capture.index as usize];
            let node = capture.node;
            let text = node.utf8_text(source_bytes).unwrap_or("");

            match capture_name {
                "function.name" | "method.name" => {
                    name = Some(text.to_string());
                    kind = if capture_name == "method.name" {
                        SymbolKind::Method
                    } else {
                        SymbolKind::Function
                    };
                }
                "class.name" => {
                    name = Some(text.to_string());
                    kind = SymbolKind::Class;
                }
                "struct.name" => {
                    name = Some(text.to_string());
                    kind = SymbolKind::Struct;
                }
                "enum.name" => {
                    name = Some(text.to_string());
                    kind = SymbolKind::Enum;
                }
                "trait.name" => {
                    name = Some(text.to_string());
                    kind = SymbolKind::Interface;
                }
                "function.def" | "method.def" | "class.def" | "struct.def" | "enum.def"
                | "trait.def" => {
                    start_line = node.start_position().row + 1;
                    end_line = node.end_position().row + 1;
                    // Extract first line as signature
                    let first_line = text.lines().next().unwrap_or(text);
                    signature = Some(first_line.to_string());
                }
                "function.docstring" | "class.docstring" => {
                    // Clean up the docstring - remove quotes and leading/trailing whitespace
                    let cleaned = text
                        .trim_start_matches("\"\"\"")
                        .trim_start_matches("'''")
                        .trim_end_matches("\"\"\"")
                        .trim_end_matches("'''")
                        .trim();
                    if !cleaned.is_empty() {
                        docstring = Some(cleaned.to_string());
                    }
                }
                _ => {}
            }
        }

        if let Some(n) = name {
            symbols.push(Symbol {
                name: n,
                kind,
                signature,
                docstring,
                start_line,
                end_line,
                children: Vec::new(),
            });
        }
    }

    // Deduplicate by name and line
    symbols.sort_by(|a, b| a.start_line.cmp(&b.start_line));
    symbols.dedup_by(|a, b| a.name == b.name && a.start_line == b.start_line);

    Ok(symbols)
}

/// Find a specific symbol by name in a file
pub fn find_symbol(source: &str, language: SupportedLanguage, symbol_name: &str) -> Result<Option<Symbol>> {
    let symbols = extract_symbols(source, language)?;
    Ok(symbols.into_iter().find(|s| s.name == symbol_name))
}

/// Get minimal context needed to use a symbol (signature + docstring)
pub fn get_symbol_context(source: &str, language: SupportedLanguage, symbol_name: &str) -> Result<Option<SymbolContext>> {
    let symbol = find_symbol(source, language, symbol_name)?;

    Ok(symbol.map(|s| SymbolContext {
        name: s.name,
        kind: s.kind,
        signature: s.signature,
        docstring: s.docstring,
        imports_needed: Vec::new(), // TODO: analyze imports
    }))
}

/// Minimal context needed to use a symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolContext {
    pub name: String,
    pub kind: SymbolKind,
    pub signature: Option<String>,
    pub docstring: Option<String>,
    pub imports_needed: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_python_functions() {
        let source = r#"
def hello(name: str) -> str:
    """Say hello to someone."""
    return f"Hello, {name}!"

def goodbye(name):
    return f"Goodbye, {name}!"

class Greeter:
    def greet(self, name):
        pass
"#;

        let symbols = extract_symbols(source, SupportedLanguage::Python).unwrap();

        let names: Vec<_> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"hello"));
        assert!(names.contains(&"goodbye"));
        assert!(names.contains(&"Greeter"));
    }

    #[test]
    fn extract_rust_items() {
        let source = r#"
fn process_data(input: &str) -> Result<String> {
    Ok(input.to_uppercase())
}

struct Config {
    name: String,
    value: i32,
}

enum Status {
    Active,
    Inactive,
}

trait Processor {
    fn process(&self) -> Result<()>;
}
"#;

        let symbols = extract_symbols(source, SupportedLanguage::Rust).unwrap();

        let names: Vec<_> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"process_data"));
        assert!(names.contains(&"Config"));
        assert!(names.contains(&"Status"));
        assert!(names.contains(&"Processor"));
    }

    #[test]
    fn find_specific_symbol() {
        let source = r#"
def foo():
    pass

def bar():
    pass

def baz():
    pass
"#;

        let symbol = find_symbol(source, SupportedLanguage::Python, "bar").unwrap();
        assert!(symbol.is_some());
        assert_eq!(symbol.unwrap().name, "bar");

        let missing = find_symbol(source, SupportedLanguage::Python, "qux").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn language_detection() {
        assert_eq!(
            SupportedLanguage::from_extension("py"),
            Some(SupportedLanguage::Python)
        );
        assert_eq!(
            SupportedLanguage::from_extension("rs"),
            Some(SupportedLanguage::Rust)
        );
        assert_eq!(
            SupportedLanguage::from_extension("ts"),
            Some(SupportedLanguage::TypeScript)
        );
        assert_eq!(SupportedLanguage::from_extension("unknown"), None);
    }

    #[test]
    fn extract_python_docstrings() {
        let source = r#"
def greet(name: str) -> str:
    """Say hello to someone.

    Args:
        name: The person's name

    Returns:
        A greeting string
    """
    return f"Hello, {name}!"

def no_docstring():
    pass
"#;

        let symbols = extract_symbols(source, SupportedLanguage::Python).unwrap();
        let greet = symbols.iter().find(|s| s.name == "greet").unwrap();

        // Check that docstring was extracted
        assert!(greet.docstring.is_some());
        let doc = greet.docstring.as_ref().unwrap();
        assert!(doc.contains("Say hello to someone"));

        // no_docstring should not have a docstring
        let no_doc = symbols.iter().find(|s| s.name == "no_docstring").unwrap();
        assert!(no_doc.docstring.is_none());
    }

    #[test]
    fn get_symbol_context_works() {
        let source = r#"
def process(data: dict) -> list:
    """Process incoming data.

    Transforms the data dictionary into a list.
    """
    return list(data.values())
"#;

        let ctx = get_symbol_context(source, SupportedLanguage::Python, "process")
            .unwrap()
            .unwrap();

        assert_eq!(ctx.name, "process");
        assert_eq!(ctx.kind, SymbolKind::Function);
        assert!(ctx.signature.is_some());
        assert!(ctx.signature.unwrap().contains("process"));
    }

    #[test]
    fn extract_class_docstrings() {
        let source = r#"
class MyClass:
    """A sample class with docstring.

    This class demonstrates docstring extraction.
    """

    def method(self):
        pass

class NoDocClass:
    pass
"#;

        let symbols = extract_symbols(source, SupportedLanguage::Python).unwrap();
        let my_class = symbols.iter().find(|s| s.name == "MyClass").unwrap();

        // Check that class docstring was extracted
        assert!(my_class.docstring.is_some());
        let doc = my_class.docstring.as_ref().unwrap();
        assert!(doc.contains("sample class"));

        // NoDocClass should not have a docstring
        let no_doc = symbols.iter().find(|s| s.name == "NoDocClass").unwrap();
        assert!(no_doc.docstring.is_none());
    }
}
