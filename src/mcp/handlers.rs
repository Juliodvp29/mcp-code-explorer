// Tool handlers — each function implements one MCP tool by calling into the
// indexer and formatting the result as a human-readable string.

use std::path::Path;

use anyhow::{bail, Result};

use crate::indexer::extractor::Symbol;
use crate::indexer::index::index_directory;
use crate::indexer::parser::TsParser;

// ── Validation ────────────────────────────────────────────────────────────────

fn require_dir(path: &str, label: &str) -> Result<()> {
    if path.trim().is_empty() {
        bail!("{label} must not be empty");
    }
    let p = Path::new(path);
    if !p.exists() {
        bail!("{label} does not exist: {path}");
    }
    if !p.is_dir() {
        bail!("{label} is not a directory: {path}");
    }
    Ok(())
}

fn require_file(path: &str, label: &str) -> Result<()> {
    if path.trim().is_empty() {
        bail!("{label} must not be empty");
    }
    Ok(()) // existence is checked when the file is actually read
}

// ── search_symbols ────────────────────────────────────────────────────────────

/// Searches the symbol index for names that contain `query` (case-insensitive).
/// Returns a formatted list of matches with file, line, and kind.
pub fn search_symbols(repo_path: &str, query: &str) -> Result<String> {
    require_dir(repo_path, "repo_path")?;
    if query.trim().is_empty() {
        bail!("query must not be empty");
    }
    let root = Path::new(repo_path);
    let symbols = index_directory(root)?;

    let query_lower = query.to_lowercase();
    let matches: Vec<&Symbol> = symbols
        .iter()
        .filter(|s| s.name.to_lowercase().contains(&query_lower))
        .collect();

    if matches.is_empty() {
        return Ok(format!("No symbols matching '{query}' found."));
    }

    let mut lines = Vec::with_capacity(matches.len());
    for sym in &matches {
        lines.push(format!(
            "{} [{}] {}:{}",
            sym.name,
            sym.kind.as_str(),
            sym.file,
            sym.line
        ));
    }
    Ok(lines.join("\n"))
}

// ── get_file_structure ────────────────────────────────────────────────────────

/// Returns all symbols defined in `file_path`, sorted by line number.
pub fn get_file_structure(repo_path: &str, file_path: &str) -> Result<String> {
    require_dir(repo_path, "repo_path")?;
    require_file(file_path, "file_path")?;
    let root = Path::new(repo_path);
    let symbols = index_directory(root)?;

    // Normalize separators for comparison.
    let target = file_path.replace('\\', "/");
    let mut file_syms: Vec<&Symbol> = symbols
        .iter()
        .filter(|s| s.file.replace('\\', "/").ends_with(&target))
        .collect();

    if file_syms.is_empty() {
        return Ok(format!("No symbols found in '{file_path}'. The file may not exist or contain no declarations."));
    }

    file_syms.sort_by_key(|s| s.line);

    let mut lines = Vec::with_capacity(file_syms.len());
    for sym in &file_syms {
        lines.push(format!("  line {:>4}  {}  [{}]", sym.line, sym.name, sym.kind.as_str()));
    }
    Ok(format!("Structure of {file_path}:\n{}", lines.join("\n")))
}

// ── find_references ───────────────────────────────────────────────────────────

/// Searches all indexed TypeScript files for occurrences of `name` as an
/// identifier in the AST (not just string matching).
pub fn find_references(repo_path: &str, name: &str) -> Result<String> {
    require_dir(repo_path, "repo_path")?;
    if name.trim().is_empty() {
        bail!("name must not be empty");
    }
    use crate::indexer::walker::Walker;

    let root = Path::new(repo_path);
    let files = Walker::for_typescript().collect(root)?;

    let mut refs: Vec<String> = Vec::new();

    for path in &files {
        let source = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => continue,
        };

        let tree = match TsParser::parse_file(path, &source) {
            Ok(t) => t,
            Err(_) => continue,
        };

        collect_identifier_refs(tree.root_node(), &source, path, name, &mut refs);
    }

    if refs.is_empty() {
        return Ok(format!("No references to '{name}' found."));
    }

    Ok(refs.join("\n"))
}

/// Walks the AST and records every `identifier` node whose text equals `name`.
fn collect_identifier_refs(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    path: &Path,
    name: &str,
    out: &mut Vec<String>,
) {
    if node.kind() == "identifier" {
        let text = std::str::from_utf8(&source[node.start_byte()..node.end_byte()])
            .unwrap_or("");
        if text == name {
            out.push(format!(
                "{}:{}",
                path.display(),
                node.start_position().row + 1
            ));
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_identifier_refs(child, source, path, name, out);
    }
}

// ── list_dependencies ─────────────────────────────────────────────────────────

/// Returns all import declarations found in `file_path`.
pub fn list_dependencies(repo_path: &str, file_path: &str) -> Result<String> {
    require_dir(repo_path, "repo_path")?;
    require_file(file_path, "file_path")?;
    let root = Path::new(repo_path);

    // Resolve the file — accept absolute paths or repo-relative paths.
    let abs_path = if Path::new(file_path).is_absolute() {
        Path::new(file_path).to_path_buf()
    } else {
        root.join(file_path)
    };

    let source = std::fs::read(&abs_path)
        .map_err(|e| anyhow::anyhow!("cannot read '{}': {e}", abs_path.display()))?;

    let tree = TsParser::parse_file(&abs_path, &source)?;

    let imports = collect_imports(tree.root_node(), &source);

    if imports.is_empty() {
        return Ok(format!("No import declarations found in '{file_path}'."));
    }

    Ok(format!(
        "Dependencies of {file_path}:\n{}",
        imports.join("\n")
    ))
}

/// Walks the AST and extracts the source string of every import declaration.
fn collect_imports(node: tree_sitter::Node<'_>, source: &[u8]) -> Vec<String> {
    let mut imports = Vec::new();
    collect_imports_inner(node, source, &mut imports);
    imports
}

fn collect_imports_inner(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    out: &mut Vec<String>,
) {
    if node.kind() == "import_statement" {
        let text =
            std::str::from_utf8(&source[node.start_byte()..node.end_byte()]).unwrap_or("");
        out.push(format!("  {}", text.trim()));
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_imports_inner(child, source, out);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_repo() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::create_dir(root.join(".git")).unwrap();

        fs::write(
            root.join("math.ts"),
            b"export function add(a: number, b: number): number { return a + b; }
export function multiply(a: number, b: number): number { return a * b; }" as &[u8],
        )
        .unwrap();

        fs::write(
            root.join("models.ts"),
            b"import { add } from './math';
export interface Shape { area(): number; }
export class Circle implements Shape {
    constructor(private radius: number) {}
    area() { return Math.PI * this.radius * this.radius; }
}
const pi = add(1, 2);" as &[u8],
        )
        .unwrap();

        fs::write(
            root.join("utils.ts"),
            b"import { Circle } from './models';
import React from 'react';
export type ID = string | number;" as &[u8],
        )
        .unwrap();

        dir
    }

    // ── search_symbols ────────────────────────────────────────────────────

    #[test]
    fn search_finds_exact_match() {
        let dir = make_repo();
        let result = search_symbols(dir.path().to_str().unwrap(), "add").unwrap();
        assert!(result.contains("add"), "must contain 'add'");
        assert!(result.contains("[function]"));
    }

    #[test]
    fn search_is_case_insensitive() {
        let dir = make_repo();
        let result = search_symbols(dir.path().to_str().unwrap(), "CIRCLE").unwrap();
        assert!(result.contains("Circle"));
    }

    #[test]
    fn search_partial_name_matches_multiple() {
        let dir = make_repo();
        let result = search_symbols(dir.path().to_str().unwrap(), "a").unwrap();
        // Both 'add' and 'Shape' contain 'a'.
        assert!(result.contains("add") || result.contains("Shape"));
    }

    #[test]
    fn search_returns_not_found_message() {
        let dir = make_repo();
        let result = search_symbols(dir.path().to_str().unwrap(), "zzznotexists").unwrap();
        assert!(result.contains("No symbols"));
    }

    // ── get_file_structure ────────────────────────────────────────────────

    #[test]
    fn file_structure_lists_symbols_in_order() {
        let dir = make_repo();
        let file = dir.path().join("math.ts");
        let result =
            get_file_structure(dir.path().to_str().unwrap(), file.to_str().unwrap()).unwrap();
        assert!(result.contains("add"));
        assert!(result.contains("multiply"));
        // add must appear before multiply (lower line number).
        assert!(result.find("add").unwrap() < result.find("multiply").unwrap());
    }

    #[test]
    fn file_structure_unknown_file_returns_message() {
        let dir = make_repo();
        let result =
            get_file_structure(dir.path().to_str().unwrap(), "nonexistent.ts").unwrap();
        assert!(result.contains("No symbols found"));
    }

    // ── find_references ───────────────────────────────────────────────────

    #[test]
    fn find_references_locates_identifier_uses() {
        let dir = make_repo();
        let result = find_references(dir.path().to_str().unwrap(), "add").unwrap();
        // 'add' is defined in math.ts and used in models.ts.
        assert!(result.contains("math.ts") || result.contains("models.ts"));
    }

    #[test]
    fn find_references_no_match_returns_message() {
        let dir = make_repo();
        let result = find_references(dir.path().to_str().unwrap(), "zzznope").unwrap();
        assert!(result.contains("No references"));
    }

    // ── list_dependencies ─────────────────────────────────────────────────

    #[test]
    fn list_deps_returns_imports() {
        let dir = make_repo();
        let file = dir.path().join("models.ts");
        let result =
            list_dependencies(dir.path().to_str().unwrap(), file.to_str().unwrap()).unwrap();
        assert!(result.contains("import"), "must contain import statement");
        assert!(result.contains("math"), "must reference ./math");
    }

    #[test]
    fn list_deps_multiple_imports() {
        let dir = make_repo();
        let file = dir.path().join("utils.ts");
        let result =
            list_dependencies(dir.path().to_str().unwrap(), file.to_str().unwrap()).unwrap();
        assert!(result.contains("models"));
        assert!(result.contains("react") || result.contains("React"));
    }

    #[test]
    fn list_deps_no_imports_returns_message() {
        let dir = make_repo();
        let file = dir.path().join("math.ts");
        let result =
            list_dependencies(dir.path().to_str().unwrap(), file.to_str().unwrap()).unwrap();
        assert!(result.contains("No import"));
    }

    #[test]
    fn list_deps_nonexistent_file_returns_error() {
        let dir = make_repo();
        let result =
            list_dependencies(dir.path().to_str().unwrap(), "/nonexistent/path/file.ts");
        assert!(result.is_err());
    }

    // ── error / validation paths ──────────────────────────────────────────

    #[test]
    fn search_empty_repo_path_returns_error() {
        let result = search_symbols("", "greet");
        assert!(result.is_err());
    }

    #[test]
    fn search_nonexistent_repo_returns_error() {
        let result = search_symbols("/nonexistent/repo/path", "greet");
        assert!(result.is_err());
    }

    #[test]
    fn search_empty_query_returns_error() {
        let dir = make_repo();
        let result = search_symbols(dir.path().to_str().unwrap(), "");
        assert!(result.is_err());
    }

    #[test]
    fn get_structure_empty_file_path_returns_error() {
        let dir = make_repo();
        let result = get_file_structure(dir.path().to_str().unwrap(), "");
        assert!(result.is_err());
    }

    #[test]
    fn find_references_empty_name_returns_error() {
        let dir = make_repo();
        let result = find_references(dir.path().to_str().unwrap(), "");
        assert!(result.is_err());
    }

    #[test]
    fn list_deps_empty_file_path_returns_error() {
        let dir = make_repo();
        let result = list_dependencies(dir.path().to_str().unwrap(), "");
        assert!(result.is_err());
    }
}
