use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::Url;

use elm_ast::file::ElmModule;
use elm_ast::module_header::ModuleHeader;
use elm_ast::parse::ParseError;
use elm_lint::collect::{self, ModuleInfo};
use elm_lint::config::Config;
use elm_lint::elm_json;
use elm_lint::rule::{LintError, ProjectContext, Rule};
use elm_lint::rules;

/// State for a single open/known document.
pub struct DocumentState {
    pub source: String,
    pub module: Option<ElmModule>,
    pub module_info: Option<ModuleInfo>,
    pub module_name: Option<String>,
    pub version: i32,
    pub lint_errors: Vec<LintError>,
    pub parse_errors: Vec<ParseError>,
}

/// Metadata about a lint rule, for hover display.
pub struct RuleInfo {
    pub description: &'static str,
    pub fixable: bool,
}

/// All mutable server state, protected by a RwLock in the backend.
#[allow(dead_code)]
pub struct ServerState {
    pub documents: HashMap<Url, DocumentState>,
    pub project_context: Option<ProjectContext>,
    pub config: Config,
    pub rules: Vec<Box<dyn Rule>>,
    pub workspace_root: PathBuf,
    pub all_module_names: Vec<String>,
    pub rule_descriptions: HashMap<String, RuleInfo>,
}

impl Default for ServerState {
    fn default() -> Self {
        ServerState {
            documents: HashMap::new(),
            project_context: None,
            config: Config::default(),
            rules: Vec::new(),
            workspace_root: PathBuf::new(),
            all_module_names: Vec::new(),
            rule_descriptions: HashMap::new(),
        }
    }
}

impl ServerState {
    /// Create a new server state, scanning the project for all .elm files.
    pub fn new(workspace_root: PathBuf) -> Self {
        // Load config.
        let config = Config::discover()
            .map(|(_path, c)| c)
            .unwrap_or_default();

        let mut rules = rules::all_rules();

        // Apply per-rule config options.
        for rule in &mut rules {
            if let Some(options) = config.rule_options(rule.name()) {
                if let Err(e) = rule.configure(options) {
                    eprintln!("Warning: error configuring rule {}: {e}", rule.name());
                }
            }
        }

        // Build rule description lookup for hover.
        let rule_descriptions = build_rule_descriptions(&rules);

        // Determine source directory.
        let src_dir = config.src.as_deref().unwrap_or("src");
        let src_path = workspace_root.join(src_dir);

        // Scan all .elm files.
        let mut documents = HashMap::new();
        let files = find_elm_files(&src_path);

        for file_path in &files {
            let Ok(source) = fs::read_to_string(file_path) else {
                continue;
            };
            let uri = file_path_to_uri(file_path);
            let doc = parse_document(source, 0);
            documents.insert(uri, doc);
        }

        let mut state = ServerState {
            documents,
            project_context: None,
            config,
            rules,
            workspace_root,
            all_module_names: Vec::new(),
            rule_descriptions,
        };

        state.rebuild_project_context();
        state
    }

    /// Reload config from disk.
    pub fn reload_config(&mut self) {
        self.config = Config::discover()
            .map(|(_path, c)| c)
            .unwrap_or_default();
    }

    /// Update a document with new source text.
    /// Returns true if the project context needs rebuilding (imports/exports changed).
    pub fn update_document(&mut self, uri: &Url, source: String, version: i32) -> bool {
        let new_doc = parse_document(source, version);

        let needs_rebuild = if let Some(old_doc) = self.documents.get(uri) {
            imports_changed(old_doc.module_info.as_ref(), new_doc.module_info.as_ref())
        } else {
            // New file — always rebuild.
            true
        };

        self.documents.insert(uri.clone(), new_doc);
        needs_rebuild
    }

    /// Rebuild the ProjectContext from all known documents.
    pub fn rebuild_project_context(&mut self) {
        let mut module_infos: HashMap<String, ModuleInfo> = HashMap::new();
        let mut all_names = Vec::new();

        for doc in self.documents.values() {
            if let (Some(name), Some(info)) = (&doc.module_name, &doc.module_info) {
                all_names.push(name.clone());
                // We need to clone the ModuleInfo to pass ownership to ProjectContext::build.
                module_infos.insert(name.clone(), clone_module_info(info));
            }
        }

        all_names.sort();
        self.all_module_names = all_names;

        let elm_json_info = elm_json::load_elm_json(&self.workspace_root).ok();
        self.project_context =
            Some(ProjectContext::build_with_elm_json(module_infos, elm_json_info));
    }

    /// Get the active rules, respecting config disable settings.
    pub fn active_rules(&self) -> Vec<&dyn Rule> {
        self.rules
            .iter()
            .filter(|r| !self.config.is_rule_disabled(r.name()))
            .map(|r| r.as_ref())
            .collect()
    }
}

/// Parse a document using error recovery and collect module info.
fn parse_document(source: String, version: i32) -> DocumentState {
    let (maybe_module, parse_errors) = elm_ast::parse_recovering(&source);

    match maybe_module {
        Some(module) => {
            let module_name = extract_module_name(&module);
            let module_info = collect::collect_module_info(&module);
            DocumentState {
                source,
                module: Some(module),
                module_info: Some(module_info),
                module_name: Some(module_name),
                version,
                lint_errors: Vec::new(),
                parse_errors,
            }
        }
        None => DocumentState {
            source,
            module: None,
            module_info: None,
            module_name: None,
            version,
            lint_errors: Vec::new(),
            parse_errors,
        },
    }
}

/// Build a lookup table of rule name → description/fixability.
fn build_rule_descriptions(rules: &[Box<dyn Rule>]) -> HashMap<String, RuleInfo> {
    // To determine fixability, we'd need to actually run a rule. Instead, we use a
    // static list of rules known to have fixes. This avoids needing synthetic test inputs.
    let fixable_rules: std::collections::HashSet<&str> = [
        "NoUnusedImports",
        "NoIfTrueFalse",
        "NoBooleanCase",
        "NoAlwaysIdentity",
        "NoRedundantCons",
        "NoUnnecessaryParens",
        "NoSinglePatternCase",
        "NoNestedNegation",
        "NoBoolOperatorSimplify",
        "NoEmptyListConcat",
        "NoListLiteralConcat",
        "NoPipelineSimplify",
        "NoNegationOfBooleanOperator",
        "NoStringConcat",
        "NoFullyAppliedPrefixOperator",
        "NoIdentityFunction",
        "NoSimpleLetBody",
        "NoMaybeMapWithNothing",
        "NoResultMapWithErr",
        // New fixable rules
        "NoUnusedParameters",
        "NoExposingAll",
        "NoImportExposingAll",
        "NoUnnecessaryPortModule",
        "NoRedundantlyQualifiedType",
        "NoEmptyLet",
        "NoEmptyRecordUpdate",
        "NoUnusedLetBinding",
        "NoUnusedVariables",
    ]
    .into_iter()
    .collect();

    rules
        .iter()
        .map(|r| {
            (
                r.name().to_string(),
                RuleInfo {
                    description: r.description(),
                    fixable: fixable_rules.contains(r.name()),
                },
            )
        })
        .collect()
}

/// Extract the dotted module name from a parsed module.
fn extract_module_name(module: &ElmModule) -> String {
    match &module.header.value {
        ModuleHeader::Normal { name, .. }
        | ModuleHeader::Port { name, .. }
        | ModuleHeader::Effect { name, .. } => name.value.join("."),
    }
}

/// Check whether the imports or exports of a module have changed.
fn imports_changed(old: Option<&ModuleInfo>, new: Option<&ModuleInfo>) -> bool {
    match (old, new) {
        (None, None) => false,
        (None, Some(_)) | (Some(_), None) => true,
        (Some(old), Some(new)) => {
            // Check import list length.
            if old.imports.len() != new.imports.len() {
                return true;
            }
            // Check exposing.
            if old.exposing.exposes_all != new.exposing.exposes_all {
                return true;
            }
            if old.exposing.exposed_values != new.exposing.exposed_values {
                return true;
            }
            if old.exposing.exposed_types != new.exposing.exposed_types {
                return true;
            }
            // Check each import.
            for (old_imp, new_imp) in old.imports.iter().zip(new.imports.iter()) {
                if old_imp.module_name != new_imp.module_name {
                    return true;
                }
                if old_imp.alias != new_imp.alias {
                    return true;
                }
            }
            false
        }
    }
}

/// Clone a ModuleInfo (manual clone since it doesn't derive Clone).
fn clone_module_info(info: &ModuleInfo) -> ModuleInfo {
    ModuleInfo {
        module_name: info.module_name.clone(),
        exposing: collect::ExposingInfo {
            exposes_all: info.exposing.exposes_all,
            exposed_values: info.exposing.exposed_values.clone(),
            exposed_types: info.exposing.exposed_types.clone(),
            exposed_types_open: info.exposing.exposed_types_open.clone(),
        },
        defined_values: info.defined_values.clone(),
        defined_types: info.defined_types.clone(),
        defined_constructors: info.defined_constructors.clone(),
        defined_ports: info.defined_ports.clone(),
        imports: info
            .imports
            .iter()
            .map(|imp| collect::ImportInfo {
                module_name: imp.module_name.clone(),
                alias: imp.alias.clone(),
                exposing: clone_import_exposing(&imp.exposing),
            })
            .collect(),
        used_values: info.used_values.clone(),
        used_qualified: info.used_qualified.clone(),
        used_types: info.used_types.clone(),
        used_qualified_types: info.used_qualified_types.clone(),
        used_constructors: info.used_constructors.clone(),
    }
}

fn clone_import_exposing(exp: &collect::ImportExposingInfo) -> collect::ImportExposingInfo {
    match exp {
        collect::ImportExposingInfo::None => collect::ImportExposingInfo::None,
        collect::ImportExposingInfo::All => collect::ImportExposingInfo::All,
        collect::ImportExposingInfo::Explicit(names) => {
            collect::ImportExposingInfo::Explicit(names.clone())
        }
    }
}

// ── File discovery ────────────────────────────────────────────────

/// Recursively find all .elm files under a directory.
pub fn find_elm_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_elm_files(dir, &mut files);
    files.sort();
    files
}

fn collect_elm_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_elm_files(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "elm") {
            files.push(path);
        }
    }
}

/// Convert a file path to a file:// URI.
pub fn file_path_to_uri(path: &Path) -> Url {
    Url::from_file_path(path).unwrap_or_else(|_| {
        // Fallback: construct manually.
        Url::parse(&format!("file://{}", path.display())).expect("valid URI")
    })
}

/// Convert a file:// URI to a file path string.
pub fn uri_to_file_path(uri: &Url) -> String {
    uri.to_file_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| uri.path().to_string())
}
