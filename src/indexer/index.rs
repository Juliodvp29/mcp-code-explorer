// Indexer — combines the file walker, parser, and symbol extractor to produce
// a complete symbol index for a repository directory.

use std::path::Path;

use anyhow::Result;
use tracing::{debug, warn};

use crate::indexer::extractor::{Extractor, Symbol};
use crate::indexer::parser::TsParser;
use crate::indexer::walker::Walker;

/// Indexes all TypeScript files under `root` and returns the collected symbols.
pub fn index_directory(root: &Path) -> Result<Vec<Symbol>> {
    let walker = Walker::for_typescript();
    let files = walker.collect(root)?;

    let mut symbols = Vec::new();

    for path in &files {
        let source = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) => {
                warn!("skipping {}: {e}", path.display());
                continue;
            }
        };

        let tree = match TsParser::parse_file(path, &source) {
            Ok(t) => t,
            Err(e) => {
                warn!("failed to parse {}: {e}", path.display());
                continue;
            }
        };

        let extractor = Extractor::new(path, &source);
        let file_symbols = extractor.extract(tree.root_node());
        debug!(
            "{}: {} symbol(s)",
            path.display(),
            file_symbols.len()
        );
        symbols.extend(file_symbols);
    }

    Ok(symbols)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_ts_repo() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Minimal git repo so .gitignore is respected.
        fs::create_dir(root.join(".git")).unwrap();
        fs::write(root.join(".gitignore"), "dist/\n").unwrap();

        fs::write(
            root.join("math.ts"),
            b"export function add(a: number, b: number) { return a + b; }" as &[u8],
        )
        .unwrap();

        fs::create_dir(root.join("models")).unwrap();
        fs::write(
            root.join("models").join("user.ts"),
            b"export interface User { id: number; name: string; }" as &[u8],
        )
        .unwrap();

        // Non-TS file — must be ignored.
        fs::write(root.join("readme.txt"), b"docs" as &[u8]).unwrap();

        // Ignored directory — must not be indexed.
        fs::create_dir(root.join("dist")).unwrap();
        fs::write(
            root.join("dist").join("bundle.ts"),
            b"export const x = 1;" as &[u8],
        )
        .unwrap();

        dir
    }

    #[test]
    fn indexes_typescript_files_in_repo() {
        let dir = make_ts_repo();
        let symbols = index_directory(dir.path()).unwrap();
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"add"), "missing add");
        assert!(names.contains(&"User"), "missing User");
    }

    #[test]
    fn does_not_index_gitignored_files() {
        let dir = make_ts_repo();
        let symbols = index_directory(dir.path()).unwrap();
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(!names.contains(&"x"), "dist/bundle.ts must be excluded");
    }

    #[test]
    fn empty_repo_returns_empty_vec() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        let symbols = index_directory(dir.path()).unwrap();
        assert!(symbols.is_empty());
    }

    #[test]
    fn indexing_project_itself_does_not_panic() {
        // This project has no .ts files, so the result must be empty and no panic.
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let symbols = index_directory(root).unwrap();
        // Symbols may be empty (no TS files) or non-empty if any exist — both are valid.
        let _ = symbols;
    }
}
