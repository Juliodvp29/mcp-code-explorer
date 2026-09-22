// TypeScript source file parser backed by tree-sitter.
// Produces a syntax tree that the symbol extractor can query.

use std::path::Path;

use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Tree};

/// Parses a single TypeScript or TSX source file and returns its syntax tree.
pub struct TsParser {
    inner: Parser,
}

impl TsParser {
    /// Creates a parser configured for TypeScript.
    pub fn new() -> Result<Self> {
        let mut inner = Parser::new();
        inner
            .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            .context("failed to load TypeScript grammar")?;
        Ok(Self { inner })
    }

    /// Creates a parser configured for TSX (TypeScript + JSX).
    pub fn new_tsx() -> Result<Self> {
        let mut inner = Parser::new();
        inner
            .set_language(&tree_sitter_typescript::LANGUAGE_TSX.into())
            .context("failed to load TSX grammar")?;
        Ok(Self { inner })
    }

    /// Selects the correct grammar variant based on the file extension,
    /// then parses `source` and returns the syntax tree.
    pub fn parse_file(path: &Path, source: &[u8]) -> Result<Tree> {
        let mut parser = match path.extension().and_then(|e| e.to_str()) {
            Some("tsx") => Self::new_tsx()?,
            _ => Self::new()?,
        };
        parser
            .inner
            .parse(source, None)
            .context("tree-sitter returned no tree (possible timeout or cancellation)")
    }
}

/// Returns the UTF-8 text covered by `node` inside `source`.
pub fn node_text<'a>(node: &Node<'_>, source: &'a [u8]) -> &'a str {
    std::str::from_utf8(&source[node.start_byte()..node.end_byte()]).unwrap_or("")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const TS_SNIPPET: &[u8] = b"
        export function greet(name: string): string {
            return `Hello, ${name}`;
        }

        export class Greeter {
            greet(name: string) { return `Hi, ${name}`; }
        }
    ";

    const TSX_SNIPPET: &[u8] = b"
        import React from 'react';
        export const App = () => <div>Hello</div>;
    ";

    #[test]
    fn parses_typescript_without_errors() {
        let path = PathBuf::from("example.ts");
        let tree = TsParser::parse_file(&path, TS_SNIPPET).unwrap();
        assert!(!tree.root_node().has_error(), "parse produced error nodes");
    }

    #[test]
    fn parses_tsx_without_errors() {
        let path = PathBuf::from("example.tsx");
        let tree = TsParser::parse_file(&path, TSX_SNIPPET).unwrap();
        assert!(!tree.root_node().has_error(), "parse produced error nodes");
    }

    #[test]
    fn root_node_is_program() {
        let path = PathBuf::from("example.ts");
        let tree = TsParser::parse_file(&path, TS_SNIPPET).unwrap();
        assert_eq!(tree.root_node().kind(), "program");
    }

    #[test]
    fn node_text_extracts_correctly() {
        let source = b"hello world";
        // Manually create a parser just to obtain a tree and a node.
        let path = PathBuf::from("x.ts");
        let tree = TsParser::parse_file(&path, source).unwrap();
        let root = tree.root_node();
        // Root covers the entire source.
        assert_eq!(node_text(&root, source), "hello world");
    }

    #[test]
    fn empty_file_parses_without_panic() {
        let path = PathBuf::from("empty.ts");
        let tree = TsParser::parse_file(&path, b"").unwrap();
        assert_eq!(tree.root_node().kind(), "program");
    }
}
