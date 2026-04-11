//! File-level lint cache. Stores file content hashes and lint results so
//! unchanged files can skip re-parsing and re-linting on subsequent runs.
//!
//! The cache is invalidated entirely when:
//! - The set of active rule names changes
//! - Any file in the project is added, removed, or modified
//!   (project-level rules depend on cross-module state)

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const CACHE_FILE: &str = ".elm-lint-cache";

/// Serializable cache entry for one file.
#[derive(Serialize, Deserialize)]
struct CacheEntry {
    hash: u64,
    errors: Vec<CachedError>,
}

/// Minimal serializable representation of a lint error.
#[derive(Serialize, Deserialize, Clone)]
pub struct CachedError {
    pub rule: String,
    pub message: String,
    pub severity: String,
    pub start_line: u32,
    pub start_col: u32,
    pub start_offset: usize,
    pub end_line: u32,
    pub end_col: u32,
    pub end_offset: usize,
    pub fixable: bool,
}

/// The full cache structure.
#[derive(Serialize, Deserialize)]
struct CacheData {
    /// Sorted list of active rule names — cache is invalid if this changes.
    rule_names: Vec<String>,
    /// Per-file cached results.
    files: HashMap<String, CacheEntry>,
}

/// In-memory cache handle.
pub struct LintCache {
    data: Option<CacheData>,
    cache_path: PathBuf,
    current_rule_names: Vec<String>,
}

impl LintCache {
    /// Load cache from disk. Returns a cache handle that may or may not have
    /// valid cached data (if the file doesn't exist or is corrupt, it starts empty).
    pub fn load(project_dir: &Path, rule_names: Vec<String>) -> Self {
        let cache_path = project_dir.join(CACHE_FILE);
        let data = fs::read(&cache_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<CacheData>(&bytes).ok())
            .filter(|d| d.rule_names == rule_names);

        LintCache {
            data,
            cache_path,
            current_rule_names: rule_names,
        }
    }

    /// Check if the cache has valid data for the given set of files.
    /// Returns true only if every file's hash matches.
    pub fn is_valid_for(&self, file_hashes: &HashMap<String, u64>) -> bool {
        let Some(data) = &self.data else {
            return false;
        };

        // Must have the exact same set of files.
        if data.files.len() != file_hashes.len() {
            return false;
        }

        for (path, hash) in file_hashes {
            match data.files.get(path) {
                Some(entry) if entry.hash == *hash => {}
                _ => return false,
            }
        }

        true
    }

    /// Get cached errors for all files. Only call if `is_valid_for` returned true.
    pub fn get_all_errors(&self) -> HashMap<String, Vec<CachedError>> {
        let Some(data) = &self.data else {
            return HashMap::new();
        };

        data.files
            .iter()
            .filter(|(_, entry)| !entry.errors.is_empty())
            .map(|(path, entry)| (path.clone(), entry.errors.clone()))
            .collect()
    }

    /// Save new results to disk.
    pub fn save(&self, file_hashes: &HashMap<String, u64>, file_errors: &HashMap<String, Vec<CachedError>>) {
        let mut files = HashMap::new();
        for (path, hash) in file_hashes {
            let errors = file_errors
                .get(path)
                .cloned()
                .unwrap_or_default();
            files.insert(path.clone(), CacheEntry { hash: *hash, errors });
        }

        let data = CacheData {
            rule_names: self.current_rule_names.clone(),
            files,
        };

        if let Ok(json) = serde_json::to_vec(&data) {
            let _ = fs::write(&self.cache_path, json);
        }
    }
}

/// Fast non-cryptographic hash of file contents.
pub fn hash_contents(contents: &[u8]) -> u64 {
    // FNV-1a 64-bit hash — fast and good enough for change detection.
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in contents {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
