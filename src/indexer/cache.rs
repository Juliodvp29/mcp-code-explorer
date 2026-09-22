// Cache module — persists the symbol index to disk and invalidates entries
// based on file modification timestamps so only changed files are re-parsed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::indexer::extractor::Symbol;

/// Format version stamped into every cache file.
/// Increment this whenever the on-disk layout changes to force a full rebuild.
const CACHE_VERSION: u32 = 1;

/// Name of the cache file written inside the repository root.
const CACHE_FILE_NAME: &str = ".mcp-index.bin";

// ── On-disk types ─────────────────────────────────────────────────────────────

/// The full contents of a cache file.
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheFile {
    /// Must match `CACHE_VERSION`; a mismatch triggers a full rebuild.
    pub version: u32,
    /// Per-file entries, keyed by the canonical file path string.
    pub entries: HashMap<String, CacheEntry>,
}

/// Cached data for a single source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Seconds since UNIX epoch at the time this entry was written.
    pub mtime_secs: u64,
    pub symbols: Vec<Symbol>,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Loads the cache stored under `repo_root`, or returns an empty cache when
/// the file does not exist or is written by an incompatible version.
pub fn load(repo_root: &Path) -> Result<CacheFile> {
    let path = cache_path(repo_root);
    if !path.exists() {
        return Ok(empty_cache());
    }

    let bytes = std::fs::read(&path)
        .with_context(|| format!("failed to read cache file {}", path.display()))?;

    let file: CacheFile = bincode::deserialize(&bytes).unwrap_or_else(|e| {
        debug!("cache deserialization failed ({e}), starting fresh");
        empty_cache()
    });

    if file.version != CACHE_VERSION {
        debug!(
            "cache version mismatch (got {}, want {}), starting fresh",
            file.version, CACHE_VERSION
        );
        return Ok(empty_cache());
    }

    Ok(file)
}

/// Writes `cache` to disk under `repo_root`.
pub fn save(repo_root: &Path, cache: &CacheFile) -> Result<()> {
    let path = cache_path(repo_root);
    let bytes = bincode::serialize(cache).context("failed to serialize cache")?;
    std::fs::write(&path, &bytes)
        .with_context(|| format!("failed to write cache file {}", path.display()))?;
    Ok(())
}

/// Returns the mtime of `path` as seconds since UNIX epoch, or 0 on error.
pub fn mtime_secs(path: &Path) -> u64 {
    path.metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Returns `true` when the cached entry for `path` is still valid
/// (the file has not been modified since the entry was written).
pub fn is_fresh(entry: &CacheEntry, path: &Path) -> bool {
    mtime_secs(path) <= entry.mtime_secs
}

/// Builds an empty cache with the current version stamp.
pub fn empty_cache() -> CacheFile {
    CacheFile {
        version: CACHE_VERSION,
        entries: HashMap::new(),
    }
}

/// Returns the path where the cache file is stored for `repo_root`.
pub fn cache_path(repo_root: &Path) -> PathBuf {
    repo_root.join(CACHE_FILE_NAME)
}

/// Returns the current wall-clock time as seconds since UNIX epoch.
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexer::extractor::SymbolKind;
    use std::fs;
    use tempfile;

    fn sample_symbol(name: &str) -> Symbol {
        Symbol {
            name: name.to_owned(),
            kind: SymbolKind::Function,
            file: "test.ts".to_owned(),
            line: 1,
        }
    }

    fn make_cache_with(symbols: Vec<Symbol>) -> CacheFile {
        let mut cache = empty_cache();
        cache.entries.insert(
            "test.ts".to_owned(),
            CacheEntry {
                mtime_secs: 1_000_000,
                symbols,
            },
        );
        cache
    }

    #[test]
    fn round_trip_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let cache = make_cache_with(vec![sample_symbol("greet"), sample_symbol("farewell")]);

        save(dir.path(), &cache).unwrap();
        let loaded = load(dir.path()).unwrap();

        let entry = loaded.entries.get("test.ts").unwrap();
        let names: Vec<&str> = entry.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"greet"));
        assert!(names.contains(&"farewell"));
    }

    #[test]
    fn load_returns_empty_when_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let cache = load(dir.path()).unwrap();
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn load_returns_empty_on_corrupt_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(cache_path(dir.path()), b"not valid bincode").unwrap();
        let cache = load(dir.path()).unwrap();
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn load_returns_empty_on_version_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = make_cache_with(vec![sample_symbol("old")]);
        cache.version = 99; // future version
        save(dir.path(), &cache).unwrap();

        let loaded = load(dir.path()).unwrap();
        assert!(loaded.entries.is_empty(), "version mismatch must reset cache");
    }

    #[test]
    fn is_fresh_detects_modified_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.ts");
        fs::write(&file, b"export function foo() {}").unwrap();

        // An entry with mtime far in the past — the file is newer.
        let entry = CacheEntry {
            mtime_secs: 0,
            symbols: vec![],
        };
        assert!(!is_fresh(&entry, &file), "stale entry must not be fresh");
    }

    #[test]
    fn is_fresh_accepts_unchanged_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.ts");
        fs::write(&file, b"export function foo() {}").unwrap();

        // An entry timestamped far in the future — the file is older.
        let entry = CacheEntry {
            mtime_secs: u64::MAX,
            symbols: vec![],
        };
        assert!(is_fresh(&entry, &file), "entry newer than file must be fresh");
    }
}
