use std::collections::{HashMap, HashSet};

use elm_ast::file::ElmModule;
use elm_ast::span::Span;

use crate::collect::{ImportExposingInfo, ModuleInfo};
use crate::elm_json::ElmJsonInfo;

/// A structured auto-fix: one or more source edits that resolve a lint error.
#[derive(Debug, Clone)]
pub struct Fix {
    pub edits: Vec<Edit>,
}

impl Fix {
    /// Create a fix with a single replacement edit.
    pub fn replace(span: Span, replacement: String) -> Self {
        Fix {
            edits: vec![Edit::Replace { span, replacement }],
        }
    }

    /// Create a fix that removes a span.
    pub fn remove(span: Span) -> Self {
        Fix {
            edits: vec![Edit::Remove { span }],
        }
    }
}

/// A single source text edit.
#[derive(Debug, Clone)]
pub enum Edit {
    /// Replace the text at `span` with `replacement`.
    Replace { span: Span, replacement: String },
    /// Insert `text` immediately after `span`.
    InsertAfter { span: Span, text: String },
    /// Remove the text at `span`.
    Remove { span: Span },
}

impl Edit {
    /// The span this edit operates on.
    pub fn span(&self) -> Span {
        match self {
            Edit::Replace { span, .. } | Edit::InsertAfter { span, .. } | Edit::Remove { span } => {
                *span
            }
        }
    }
}

/// Severity level for a lint error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// A lint error reported by a rule.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct LintError {
    /// The rule that produced this error.
    pub rule: &'static str,
    /// Severity level.
    pub severity: Severity,
    /// Human-readable error message.
    pub message: String,
    /// Source location of the error.
    pub span: Span,
    /// Optional auto-fix.
    pub fix: Option<Fix>,
}

/// Pre-computed project-wide cross-module data.
pub struct ProjectContext {
    /// ModuleInfo for every module in the project, keyed by dotted module name.
    pub modules: HashMap<String, ModuleInfo>,
    /// For each module name, the set of names that other modules import from it.
    pub imported_from: HashMap<String, HashSet<String>>,
    /// For each module name, the set of module names that import it.
    pub importers: HashMap<String, HashSet<String>>,
    /// Parsed elm.json info (dependencies), if available.
    pub elm_json: Option<ElmJsonInfo>,
}

impl ProjectContext {
    /// Build a `ProjectContext` from a map of module names to their collected info.
    pub fn build(modules: HashMap<String, ModuleInfo>) -> Self {
        Self::build_with_elm_json(modules, None)
    }

    /// Build a `ProjectContext` with optional elm.json info.
    pub fn build_with_elm_json(
        modules: HashMap<String, ModuleInfo>,
        elm_json: Option<ElmJsonInfo>,
    ) -> Self {
        let mut imported_from: HashMap<String, HashSet<String>> = HashMap::new();
        let mut importers: HashMap<String, HashSet<String>> = HashMap::new();

        for info in modules.values() {
            let importer_name = info.module_name.join(".");
            for imp in &info.imports {
                let imp_module = imp.module_name.join(".");

                // Track that importer_name imports from imp_module.
                importers
                    .entry(imp_module.clone())
                    .or_default()
                    .insert(importer_name.clone());

                // Track which names are imported.
                match &imp.exposing {
                    ImportExposingInfo::None => {}
                    ImportExposingInfo::All => {
                        // Conservative: if anyone does `import Foo exposing (..)`,
                        // mark all of Foo's exports as used to avoid false positives.
                        if let Some(target) = modules.get(&imp_module) {
                            let names = imported_from.entry(imp_module.clone()).or_default();
                            for v in &target.exposing.exposed_values {
                                names.insert(v.clone());
                            }
                            for t in &target.exposing.exposed_types {
                                names.insert(t.clone());
                            }
                        }
                    }
                    ImportExposingInfo::Explicit(names) => {
                        imported_from
                            .entry(imp_module.clone())
                            .or_default()
                            .extend(names.iter().cloned());
                    }
                }
            }
        }

        ProjectContext {
            modules,
            imported_from,
            importers,
            elm_json,
        }
    }
}

/// The context passed to rules, containing the module and project-level info.
#[allow(dead_code)]
pub struct LintContext<'a> {
    pub module: &'a ElmModule,
    pub source: &'a str,
    pub file_path: &'a str,
    /// All module names in the project (for cross-module checks).
    pub project_modules: &'a [String],
    /// This module's collected info (definitions, references, exports).
    pub module_info: Option<&'a ModuleInfo>,
    /// Full project context for cross-module rules.
    pub project: Option<&'a ProjectContext>,
}

/// A lint rule.
///
/// Each rule inspects a parsed module and returns zero or more errors.
pub trait Rule: Send + Sync {
    /// The rule's unique identifier, e.g. "NoUnusedImports".
    fn name(&self) -> &'static str;

    /// A short description of what this rule checks.
    fn description(&self) -> &'static str;

    /// Default severity for this rule. Override for rules that should be errors by default.
    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    /// Apply per-rule options from `elm-assist.toml`. Called once at startup.
    /// The value is the TOML table for this rule, e.g. for
    /// `[rules.NoMaxLineLength] max_length = 100` the value is `{ max_length = 100 }`.
    fn configure(&mut self, _options: &toml::Value) -> Result<(), String> {
        Ok(())
    }

    /// Run the rule against a module and return any findings.
    fn check(&self, ctx: &LintContext) -> Vec<LintError>;
}
