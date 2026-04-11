//! Parse `elm.json` to extract dependency information and resolve package
//! modules from the Elm package cache (`~/.elm/0.19.1/packages/`).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::{env, fs};

use serde::Deserialize;

/// Parsed dependency info from elm.json, with resolved package modules.
#[derive(Debug)]
pub struct ElmJsonInfo {
    /// Direct dependencies: package name -> version constraint string.
    pub direct_deps: HashMap<String, String>,
    /// Whether this is an application (vs a package).
    pub is_application: bool,
    /// Resolved package → exposed modules mapping. Built from the Elm package
    /// cache when available, with hardcoded fallbacks for common packages.
    pub package_modules: HashMap<String, Vec<String>>,
}

/// Errors that can occur when loading elm.json.
#[derive(Debug)]
pub enum ElmJsonError {
    Io(std::io::Error),
    Parse(serde_json::Error),
}

impl std::fmt::Display for ElmJsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ElmJsonError::Io(e) => write!(f, "could not read elm.json: {e}"),
            ElmJsonError::Parse(e) => write!(f, "could not parse elm.json: {e}"),
        }
    }
}

/// Load and parse elm.json from the given directory (or its parents),
/// then resolve package modules from the Elm cache.
pub fn load_elm_json(start_dir: &Path) -> Result<ElmJsonInfo, ElmJsonError> {
    let path = find_elm_json(start_dir).ok_or_else(|| {
        ElmJsonError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "elm.json not found",
        ))
    })?;
    let contents = fs::read_to_string(&path).map_err(ElmJsonError::Io)?;
    let mut info = parse_elm_json(&contents)?;

    // Resolve package modules from the Elm cache.
    let elm_home = resolve_elm_home();
    info.package_modules = resolve_all_package_modules(&info.direct_deps, elm_home.as_deref());

    Ok(info)
}

/// Walk up from `start_dir` looking for elm.json.
fn find_elm_json(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let candidate = dir.join("elm.json");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Parse elm.json content string into ElmJsonInfo (without resolved modules).
pub fn parse_elm_json(contents: &str) -> Result<ElmJsonInfo, ElmJsonError> {
    // Try application format first, then package format.
    if let Ok(app) = serde_json::from_str::<ApplicationElmJson>(contents) {
        if app.type_field == "application" {
            return Ok(ElmJsonInfo {
                direct_deps: app.dependencies.direct,
                is_application: true,
                package_modules: HashMap::new(),
            });
        }
    }
    if let Ok(pkg) = serde_json::from_str::<PackageElmJson>(contents) {
        if pkg.type_field == "package" {
            return Ok(ElmJsonInfo {
                direct_deps: pkg.dependencies,
                is_application: false,
                package_modules: HashMap::new(),
            });
        }
    }
    Err(ElmJsonError::Parse(
        serde_json::from_str::<()>(contents).unwrap_err(),
    ))
}

// ── elm.json formats ─────────────────────────────────────────────────

/// Application elm.json format.
#[derive(Deserialize)]
struct ApplicationElmJson {
    #[serde(rename = "type")]
    type_field: String,
    dependencies: AppDependencies,
}

#[derive(Deserialize)]
struct AppDependencies {
    direct: HashMap<String, String>,
}

/// Package elm.json format.
#[derive(Deserialize)]
struct PackageElmJson {
    #[serde(rename = "type")]
    type_field: String,
    dependencies: HashMap<String, String>,
}

// ── Elm package cache resolution ─────────────────────────────────────

/// Resolve `ELM_HOME`. Checks the `ELM_HOME` env var, then falls back to
/// `~/.elm`. Returns `None` if the directory doesn't exist.
fn resolve_elm_home() -> Option<PathBuf> {
    let path = if let Ok(home) = env::var("ELM_HOME") {
        PathBuf::from(home)
    } else {
        dirs_fallback_home()?.join(".elm")
    };

    if path.is_dir() {
        Some(path)
    } else {
        None
    }
}

/// Get the user's home directory without pulling in the `dirs` crate.
fn dirs_fallback_home() -> Option<PathBuf> {
    env::var("HOME")
        .ok()
        .map(PathBuf::from)
}

/// For each dependency, try to read its `exposed-modules` from the Elm cache.
/// Packages not found in the cache are silently skipped (no false positives).
fn resolve_all_package_modules(
    deps: &HashMap<String, String>,
    elm_home: Option<&Path>,
) -> HashMap<String, Vec<String>> {
    let elm_home = match elm_home {
        Some(path) => path,
        None => return HashMap::new(),
    };

    let mut result = HashMap::new();
    for (pkg_name, version_str) in deps {
        if let Some(modules) = read_package_modules_from_cache(elm_home, pkg_name, version_str) {
            result.insert(pkg_name.clone(), modules);
        }
    }
    result
}

/// Read a single package's exposed modules from the Elm cache.
///
/// For applications, `version_str` is an exact version like `"1.1.3"`.
/// For packages, it's a constraint like `"1.0.0 <= v < 2.0.0"` — in that case
/// we scan the directory for the highest installed version.
fn read_package_modules_from_cache(
    elm_home: &Path,
    pkg_name: &str,
    version_str: &str,
) -> Option<Vec<String>> {
    // Package name is "author/name" — split into path segments.
    let (author, name) = pkg_name.split_once('/')?;
    let pkg_dir = elm_home
        .join("0.19.1")
        .join("packages")
        .join(author)
        .join(name);

    // Resolve version: try exact first, then scan for highest.
    let version_dir = if is_exact_version(version_str) {
        let dir = pkg_dir.join(version_str);
        if dir.is_dir() {
            dir
        } else {
            return None;
        }
    } else {
        find_highest_installed_version(&pkg_dir)?
    };

    let elm_json_path = version_dir.join("elm.json");
    let contents = fs::read_to_string(elm_json_path).ok()?;
    parse_exposed_modules(&contents)
}

/// Check if a version string is an exact semver (e.g., "1.1.3") rather than a
/// constraint (e.g., "1.0.0 <= v < 2.0.0").
fn is_exact_version(s: &str) -> bool {
    // Exact versions contain only digits and dots.
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// Find the highest installed version directory under a package path.
fn find_highest_installed_version(pkg_dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(pkg_dir).ok()?;
    let mut versions: Vec<(Vec<u32>, PathBuf)> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if let Some(parsed) = parse_semver(&name_str) {
                versions.push((parsed, path));
            }
        }
    }

    versions.sort_by(|a, b| a.0.cmp(&b.0));
    versions.into_iter().last().map(|(_, path)| path)
}

/// Parse a semver string like "1.2.3" into comparable parts.
fn parse_semver(s: &str) -> Option<Vec<u32>> {
    let parts: Vec<u32> = s.split('.').filter_map(|p| p.parse().ok()).collect();
    if parts.len() == 3 {
        Some(parts)
    } else {
        None
    }
}

/// Parse the `exposed-modules` field from a package's elm.json.
///
/// Handles both formats:
/// - Flat list: `["Module.A", "Module.B"]`
/// - Categorized: `{ "Category": ["Module.A"], "Other": ["Module.B"] }`
fn parse_exposed_modules(contents: &str) -> Option<Vec<String>> {
    let value: serde_json::Value = serde_json::from_str(contents).ok()?;
    let exposed = value.get("exposed-modules")?;

    match exposed {
        serde_json::Value::Array(arr) => {
            let modules: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            if modules.is_empty() {
                None
            } else {
                Some(modules)
            }
        }
        serde_json::Value::Object(obj) => {
            let mut modules = Vec::new();
            for value in obj.values() {
                if let serde_json::Value::Array(arr) = value {
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            modules.push(s.to_string());
                        }
                    }
                }
            }
            if modules.is_empty() {
                None
            } else {
                Some(modules)
            }
        }
        _ => None,
    }
}

/// Given a set of imported module names and a resolved package→modules mapping,
/// return the set of package names that have at least one module imported.
pub fn packages_used_by_imports(
    imported_modules: &HashSet<String>,
    package_modules: &HashMap<String, Vec<String>>,
) -> HashSet<String> {
    let mut used = HashSet::new();
    for (pkg, modules) in package_modules {
        for module in modules {
            if imported_modules.contains(module) {
                used.insert(pkg.clone());
                break;
            }
        }
    }
    used
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_application_elm_json() {
        let json = r#"{
            "type": "application",
            "source-directories": ["src"],
            "elm-version": "0.19.1",
            "dependencies": {
                "direct": {
                    "elm/core": "1.0.5",
                    "elm/html": "1.0.0",
                    "elm/json": "1.1.3"
                },
                "indirect": {
                    "elm/virtual-dom": "1.0.3"
                }
            },
            "test-dependencies": {
                "direct": {},
                "indirect": {}
            }
        }"#;

        let info = parse_elm_json(json).unwrap();
        assert!(info.is_application);
        assert_eq!(info.direct_deps.len(), 3);
        assert!(info.direct_deps.contains_key("elm/core"));
        assert!(info.direct_deps.contains_key("elm/html"));
        assert!(info.direct_deps.contains_key("elm/json"));
    }

    #[test]
    fn parse_package_elm_json() {
        let json = r#"{
            "type": "package",
            "name": "author/my-package",
            "summary": "A package",
            "license": "BSD-3-Clause",
            "version": "1.0.0",
            "exposed-modules": ["MyModule"],
            "elm-version": "0.19.0 <= v < 0.20.0",
            "dependencies": {
                "elm/core": "1.0.0 <= v < 2.0.0",
                "elm/json": "1.0.0 <= v < 2.0.0"
            },
            "test-dependencies": {}
        }"#;

        let info = parse_elm_json(json).unwrap();
        assert!(!info.is_application);
        assert_eq!(info.direct_deps.len(), 2);
        assert!(info.direct_deps.contains_key("elm/core"));
    }

    #[test]
    fn parse_exposed_modules_flat_list() {
        let json = r#"{
            "type": "package",
            "exposed-modules": ["Json.Decode", "Json.Encode"]
        }"#;
        let modules = parse_exposed_modules(json).unwrap();
        assert_eq!(modules, vec!["Json.Decode", "Json.Encode"]);
    }

    #[test]
    fn parse_exposed_modules_categorized() {
        let json = r#"{
            "type": "package",
            "exposed-modules": {
                "Decode": ["Json.Decode"],
                "Encode": ["Json.Encode"]
            }
        }"#;
        let modules = parse_exposed_modules(json).unwrap();
        assert!(modules.contains(&"Json.Decode".to_string()));
        assert!(modules.contains(&"Json.Encode".to_string()));
    }

    #[test]
    fn is_exact_version_works() {
        assert!(is_exact_version("1.0.0"));
        assert!(is_exact_version("1.1.3"));
        assert!(!is_exact_version("1.0.0 <= v < 2.0.0"));
        assert!(!is_exact_version(""));
    }

    #[test]
    fn parse_semver_works() {
        assert_eq!(parse_semver("1.2.3"), Some(vec![1, 2, 3]));
        assert_eq!(parse_semver("0.19.1"), Some(vec![0, 19, 1]));
        assert_eq!(parse_semver("abc"), None);
        assert_eq!(parse_semver("1.2"), None);
    }

    #[test]
    fn packages_used_by_imports_finds_matches() {
        let mut pkg_modules = HashMap::new();
        pkg_modules.insert(
            "elm/json".to_string(),
            vec!["Json.Decode".to_string(), "Json.Encode".to_string()],
        );
        pkg_modules.insert("elm/html".to_string(), vec!["Html".to_string()]);
        pkg_modules.insert("elm/http".to_string(), vec!["Http".to_string()]);

        let mut imports = HashSet::new();
        imports.insert("Json.Decode".to_string());
        imports.insert("Html".to_string());

        let used = packages_used_by_imports(&imports, &pkg_modules);
        assert!(used.contains("elm/json"));
        assert!(used.contains("elm/html"));
        assert!(!used.contains("elm/http"));
    }

    #[test]
    fn packages_used_by_imports_no_match() {
        let pkg_modules = HashMap::new();
        let imports = HashSet::new();
        let used = packages_used_by_imports(&imports, &pkg_modules);
        assert!(used.is_empty());
    }

    #[test]
    fn resolve_returns_empty_without_elm_home() {
        let mut deps = HashMap::new();
        deps.insert("elm/json".to_string(), "1.1.3".to_string());

        let result = resolve_all_package_modules(&deps, None);
        assert!(result.is_empty());
    }
}
