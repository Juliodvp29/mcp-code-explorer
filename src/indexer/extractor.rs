// Symbol extractor — walks a tree-sitter AST and collects named declarations
// from TypeScript source files.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tree_sitter::Node;

use crate::indexer::parser::node_text;

// ── Symbol types ──────────────────────────────────────────────────────────────

/// The kind of a declared symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Class,
    Interface,
    TypeAlias,
    Enum,
    Variable,
    ArrowFunction,
}

impl SymbolKind {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Class => "class",
            SymbolKind::Interface => "interface",
            SymbolKind::TypeAlias => "type_alias",
            SymbolKind::Enum => "enum",
            SymbolKind::Variable => "variable",
            SymbolKind::ArrowFunction => "arrow_function",
        }
    }
}

/// A named declaration found in a source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    /// Absolute or repo-relative path of the source file.
    pub file: String,
    /// 1-based line number of the declaration.
    pub line: usize,
}

// ── Extractor ─────────────────────────────────────────────────────────────────

/// Extracts all top-level and exported symbols from a parsed syntax tree.
pub struct Extractor<'src> {
    source: &'src [u8],
    file: String,
}

impl<'src> Extractor<'src> {
    pub fn new(path: &Path, source: &'src [u8]) -> Self {
        Self {
            source,
            file: path.to_string_lossy().into_owned(),
        }
    }

    /// Walks the entire tree rooted at `root` and returns all symbols found.
    pub fn extract(&self, root: Node<'_>) -> Vec<Symbol> {
        let mut symbols = Vec::new();
        self.visit(root, &mut symbols);
        symbols
    }

    fn visit(&self, node: Node<'_>, out: &mut Vec<Symbol>) {
        match node.kind() {
            "function_declaration" | "function_signature" => {
                if let Some(sym) = self.named_symbol(&node, SymbolKind::Function, "name") {
                    out.push(sym);
                }
            }

            "class_declaration" => {
                if let Some(sym) = self.named_symbol(&node, SymbolKind::Class, "name") {
                    out.push(sym);
                }
            }

            "interface_declaration" => {
                if let Some(sym) = self.named_symbol(&node, SymbolKind::Interface, "name") {
                    out.push(sym);
                }
            }

            "type_alias_declaration" => {
                if let Some(sym) = self.named_symbol(&node, SymbolKind::TypeAlias, "name") {
                    out.push(sym);
                }
            }

            "enum_declaration" => {
                if let Some(sym) = self.named_symbol(&node, SymbolKind::Enum, "name") {
                    out.push(sym);
                }
            }

            // `export const Foo = ...` or `const Foo = ...`
            "lexical_declaration" | "variable_declaration" => {
                self.visit_variable_declaration(&node, out);
            }

            // Recurse into export wrappers and the program root.
            "export_statement" | "program" | "statement_block" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    self.visit(child, out);
                }
            }

            _ => {}
        }
    }

    fn visit_variable_declaration(&self, node: &Node<'_>, out: &mut Vec<Symbol>) {
        let mut cursor = node.walk();
        for declarator in node.children(&mut cursor) {
            if declarator.kind() != "variable_declarator" {
                continue;
            }

            let name_node = match declarator.child_by_field_name("name") {
                Some(n) => n,
                None => continue,
            };

            let value_node = declarator.child_by_field_name("value");
            let kind = match value_node.map(|n| n.kind()) {
                Some("arrow_function") => SymbolKind::ArrowFunction,
                _ => SymbolKind::Variable,
            };

            out.push(Symbol {
                name: node_text(&name_node, self.source).to_owned(),
                kind,
                file: self.file.clone(),
                line: name_node.start_position().row + 1,
            });
        }
    }

    fn named_symbol(&self, node: &Node<'_>, kind: SymbolKind, field: &str) -> Option<Symbol> {
        let name_node = node.child_by_field_name(field)?;
        Some(Symbol {
            name: node_text(&name_node, self.source).to_owned(),
            kind,
            file: self.file.clone(),
            line: name_node.start_position().row + 1,
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::indexer::parser::TsParser;

    fn extract(source: &[u8]) -> Vec<Symbol> {
        let path = PathBuf::from("test.ts");
        let tree = TsParser::parse_file(&path, source).unwrap();
        let extractor = Extractor::new(&path, source);
        extractor.extract(tree.root_node())
    }

    fn names(symbols: &[Symbol]) -> Vec<&str> {
        symbols.iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn extracts_function_declaration() {
        let src = b"function greet(name: string): string { return name; }";
        let syms = extract(src);
        assert!(names(&syms).contains(&"greet"));
        assert_eq!(syms[0].kind, SymbolKind::Function);
    }

    #[test]
    fn extracts_class_declaration() {
        let src = b"class Animal { name: string = ''; }";
        let syms = extract(src);
        assert!(names(&syms).contains(&"Animal"));
        assert_eq!(syms[0].kind, SymbolKind::Class);
    }

    #[test]
    fn extracts_interface_declaration() {
        let src = b"interface Shape { area(): number; }";
        let syms = extract(src);
        assert!(names(&syms).contains(&"Shape"));
        assert_eq!(syms[0].kind, SymbolKind::Interface);
    }

    #[test]
    fn extracts_type_alias() {
        let src = b"type ID = string | number;";
        let syms = extract(src);
        assert!(names(&syms).contains(&"ID"));
        assert_eq!(syms[0].kind, SymbolKind::TypeAlias);
    }

    #[test]
    fn extracts_enum() {
        let src = b"enum Direction { Up, Down, Left, Right }";
        let syms = extract(src);
        assert!(names(&syms).contains(&"Direction"));
        assert_eq!(syms[0].kind, SymbolKind::Enum);
    }

    #[test]
    fn extracts_arrow_function_assigned_to_const() {
        let src = b"const greet = (name: string) => `Hello, ${name}`;";
        let syms = extract(src);
        assert!(names(&syms).contains(&"greet"));
        assert_eq!(syms[0].kind, SymbolKind::ArrowFunction);
    }

    #[test]
    fn extracts_exported_declarations() {
        let src = b"
            export function add(a: number, b: number): number { return a + b; }
            export class Counter {}
            export interface Countable { count(): number; }
        ";
        let syms = extract(src);
        let ns = names(&syms);
        assert!(ns.contains(&"add"));
        assert!(ns.contains(&"Counter"));
        assert!(ns.contains(&"Countable"));
    }

    #[test]
    fn line_numbers_are_one_based() {
        let src = b"\nfunction foo() {}\n";
        let syms = extract(src);
        let foo = syms.iter().find(|s| s.name == "foo").unwrap();
        assert_eq!(foo.line, 2);
    }

    #[test]
    fn empty_file_produces_no_symbols() {
        let syms = extract(b"");
        assert!(syms.is_empty());
    }
}
