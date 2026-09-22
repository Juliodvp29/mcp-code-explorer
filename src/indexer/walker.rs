// Walker — traverses a directory tree respecting .gitignore rules and returns
// paths that match a configured set of file extensions.

use std::path::{Path, PathBuf};

use anyhow::Result;
use ignore::WalkBuilder;

/// Walks a directory tree and collects files whose extension is in the allow-list.
pub struct Walker {
    extensions: Vec<&'static str>,
}

impl Walker {
    /// Creates a walker restricted to TypeScript source files.
    pub fn for_typescript() -> Self {
        Self {
            extensions: vec!["ts", "tsx"],
        }
    }

    /// Returns all matching files under `root`, respecting `.gitignore` rules.
    /// Symbolic links are not followed. Hidden directories are skipped unless
    /// they contain a `.gitignore` that explicitly includes entries.
    pub fn collect(&self, root: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        for entry in WalkBuilder::new(root)
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .follow_links(false)
            .build()
        {
            let entry = entry?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            if self.matches_extension(path) {
                files.push(path.to_path_buf());
            }
        }

        Ok(files)
    }

    fn matches_extension(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| self.extensions.contains(&e))
            .unwrap_or(false)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_temp_tree() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // A .git directory is required for the ignore crate to apply .gitignore rules.
        fs::create_dir(root.join(".git")).unwrap();

        // TS files that should be collected.
        fs::write(root.join("app.ts"), "export {}").unwrap();
        fs::write(root.join("comp.tsx"), "export {}").unwrap();

        // Sub-directory with a TS file.
        fs::create_dir(root.join("lib")).unwrap();
        fs::write(root.join("lib").join("utils.ts"), "export {}").unwrap();

        // Files that must be excluded.
        fs::write(root.join("index.js"), "").unwrap();
        fs::write(root.join("README.md"), "").unwrap();
        fs::write(root.join("data.json"), "").unwrap();

        // Simulated build output that .gitignore would exclude.
        fs::create_dir(root.join("dist")).unwrap();
        fs::write(root.join("dist").join("bundle.ts"), "").unwrap();
        fs::write(root.join(".gitignore"), "dist/\n").unwrap();

        dir
    }

    #[test]
    fn collects_ts_and_tsx_files() {
        let dir = make_temp_tree();
        let walker = Walker::for_typescript();
        let mut files = walker.collect(dir.path()).unwrap();
        files.sort();

        let names: Vec<&str> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();

        assert!(names.contains(&"app.ts"), "missing app.ts");
        assert!(names.contains(&"comp.tsx"), "missing comp.tsx");
        assert!(names.contains(&"utils.ts"), "missing utils.ts");
    }

    #[test]
    fn excludes_non_ts_files() {
        let dir = make_temp_tree();
        let walker = Walker::for_typescript();
        let files = walker.collect(dir.path()).unwrap();

        let names: Vec<&str> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();

        assert!(!names.contains(&"index.js"), "js file must be excluded");
        assert!(!names.contains(&"README.md"), "md file must be excluded");
        assert!(!names.contains(&"data.json"), "json file must be excluded");
    }

    #[test]
    fn respects_gitignore_exclusions() {
        let dir = make_temp_tree();
        let walker = Walker::for_typescript();
        let files = walker.collect(dir.path()).unwrap();

        let names: Vec<&str> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();

        assert!(
            !names.contains(&"bundle.ts"),
            "dist/bundle.ts must be excluded by .gitignore"
        );
    }

    #[test]
    fn empty_directory_returns_empty_vec() {
        let dir = tempfile::tempdir().unwrap();
        let walker = Walker::for_typescript();
        let files = walker.collect(dir.path()).unwrap();
        assert!(files.is_empty());
    }
}
