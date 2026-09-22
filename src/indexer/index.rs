// Indexer — combines the file walker, parser, and symbol extractor to produce
// a complete symbol index for a repository directory.
// Uses a disk cache keyed by file mtime to skip re-parsing unchanged files.

use std::path::Path;

use anyhow::Result;
use tracing::{debug, warn};

use crate::indexer::cache::{self, CacheEntry};
use crate::indexer::extractor::{Extractor, Symbol};
use crate::indexer::parser::TsParser;
use crate::indexer::walker::Walker;

/// Indexes all TypeScript files under `root`.
///
/// On the first call the full directory is parsed and a cache is written to
/// `<root>/.mcp-index.bin`. On subsequent calls only files whose mtime has
/// changed since the last run are re-parsed; all other symbols are served from
/// the cache, making repeated runs significantly faster.
pub fn index_directory(root: &Path) -> Result<Vec<Symbol>> {
    let walker = Walker::for_typescript();
    let files = walker.collect(root)?;

    let mut disk_cache = cache::load(root)?;
    let mut any_changed = false;

    let mut all_symbols: Vec<Symbol> = Vec::new();

    for path in &files {
        let path_key = path.to_string_lossy().into_owned();

        // Check if the cached entry is still valid.
        if let Some(entry) = disk_cache.entries.get(&path_key) {
            if cache::is_fresh(entry, path) {
                debug!("cache hit: {}", path.display());
                all_symbols.extend(entry.symbols.clone());
                continue;
            }
        }

        // Cache miss or stale — parse the file.
        debug!("cache miss: {}", path.display());
        any_changed = true;

        let source = match std::fs::read(path) {
            Ok(b) => b,
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
        let symbols = extractor.extract(tree.root_node());
        debug!("{}: {} symbol(s)", path.display(), symbols.len());

        // Update the cache entry with the new mtime and symbols.
        disk_cache.entries.insert(
            path_key,
            CacheEntry {
                mtime_secs: cache::mtime_secs(path),
                symbols: symbols.clone(),
            },
        );

        all_symbols.extend(symbols);
    }

    // Remove entries for files that no longer exist.
    let live_keys: std::collections::HashSet<String> = files
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();

    let before = disk_cache.entries.len();
    disk_cache.entries.retain(|k, _| live_keys.contains(k));
    if disk_cache.entries.len() != before {
        any_changed = true;
    }

    // Persist the updated cache only when something changed.
    if any_changed {
        if let Err(e) = cache::save(root, &disk_cache) {
            warn!("failed to save cache: {e}");
        }
    }

    Ok(all_symbols)
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

        fs::write(root.join("readme.txt"), b"docs" as &[u8]).unwrap();

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
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let symbols = index_directory(root).unwrap();
        let _ = symbols;
    }

    #[test]
    fn second_run_serves_symbols_from_cache() {
        let dir = make_ts_repo();

        // First run — populates the cache.
        let first = index_directory(dir.path()).unwrap();
        assert!(!first.is_empty());

        // Second run — cache file must exist now.
        assert!(cache::cache_path(dir.path()).exists(), "cache file must be written");

        let second = index_directory(dir.path()).unwrap();

        // Both runs must return the same set of symbol names.
        let mut first_names: Vec<&str> = first.iter().map(|s| s.name.as_str()).collect();
        let mut second_names: Vec<&str> = second.iter().map(|s| s.name.as_str()).collect();
        first_names.sort();
        second_names.sort();
        assert_eq!(first_names, second_names, "cached run must return identical symbols");
    }

    #[test]
    fn modified_file_is_reindexed() {
        let dir = make_ts_repo();

        // First run — populate cache.
        index_directory(dir.path()).unwrap();

        // Overwrite math.ts with a new symbol, then backdate its cache entry
        // so the indexer considers it stale regardless of OS mtime resolution.
        fs::write(
            dir.path().join("math.ts"),
            b"export function multiply(a: number, b: number) { return a * b; }" as &[u8],
        )
        .unwrap();

        // Force the cached mtime to 0 so is_fresh() always returns false.
        let mut disk_cache = cache::load(dir.path()).unwrap();
        let math_key = dir
            .path()
            .join("math.ts")
            .to_string_lossy()
            .into_owned();
        if let Some(entry) = disk_cache.entries.get_mut(&math_key) {
            entry.mtime_secs = 0;
        }
        cache::save(dir.path(), &disk_cache).unwrap();

        let symbols = index_directory(dir.path()).unwrap();
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"multiply"), "new symbol must be indexed");
    }
}
