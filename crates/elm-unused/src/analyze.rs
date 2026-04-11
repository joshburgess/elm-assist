use std::collections::{HashMap, HashSet};

use crate::collect::{ImportExposingInfo, ModuleInfo};

/// A single finding from the dead code analysis.
#[derive(Debug, Clone)]
pub struct Finding {
    pub module_name: String,
    pub kind: FindingKind,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum FindingKind {
    /// An import statement where nothing from the module is used.
    UnusedImport,
    /// A specific name in an exposing list that is never used.
    UnusedImportExposing,
    /// A function defined but never referenced internally or exported.
    UnusedFunction,
    /// An exported function that no other module in the project imports.
    UnusedExport,
    /// A custom type constructor that is never constructed or pattern-matched.
    UnusedConstructor,
    /// A type alias or custom type that is never referenced.
    UnusedType,
}

impl FindingKind {
    pub fn label(&self) -> &'static str {
        match self {
            FindingKind::UnusedImport => "unused import",
            FindingKind::UnusedImportExposing => "unused import exposing",
            FindingKind::UnusedFunction => "unused function",
            FindingKind::UnusedExport => "unused export",
            FindingKind::UnusedConstructor => "unused constructor",
            FindingKind::UnusedType => "unused type",
        }
    }
}

/// Analyze all modules for unused code.
pub fn analyze(modules: &HashMap<String, ModuleInfo>) -> Vec<Finding> {
    let mut findings = Vec::new();

    // Build a map of what each module imports from others.
    let mut imported_from: HashMap<String, HashSet<String>> = HashMap::new();
    for info in modules.values() {
        for imp in &info.imports {
            let imp_module = imp.module_name.join(".");
            imported_from
                .entry(imp_module)
                .or_default()
                .extend(get_imported_names(imp));
        }
    }

    for info in modules.values() {
        let mod_name = info.module_name.join(".");

        // ── Unused imports ───────────────────────────────────────
        for imp in &info.imports {
            let imp_module = imp.module_name.join(".");
            let imp_alias = imp.alias.as_deref().unwrap_or(&imp_module);

            // Check if any qualified reference uses this module/alias.
            let qualified_used = info
                .used_qualified
                .iter()
                .chain(info.used_qualified_types.iter())
                .any(|(m, _)| m == imp_alias || m == &imp_module);

            // Check if any exposed name is used.
            let exposed_used = match &imp.exposing {
                ImportExposingInfo::None => false,
                ImportExposingInfo::All => true, // can't easily check
                ImportExposingInfo::Explicit(names) => names.iter().any(|n| {
                    info.used_values.contains(n)
                        || info.used_types.contains(n)
                        || info.used_constructors.contains(n)
                }),
            };

            if !qualified_used && !exposed_used {
                findings.push(Finding {
                    module_name: mod_name.clone(),
                    kind: FindingKind::UnusedImport,
                    name: imp_module.clone(),
                });
            }

            // ── Unused import exposings ──────────────────────────
            if let ImportExposingInfo::Explicit(names) = &imp.exposing {
                for name in names {
                    let is_used = info.used_values.contains(name)
                        || info.used_types.contains(name)
                        || info.used_constructors.contains(name);
                    if !is_used {
                        findings.push(Finding {
                            module_name: mod_name.clone(),
                            kind: FindingKind::UnusedImportExposing,
                            name: format!("{imp_module}.{name}"),
                        });
                    }
                }
            }
        }

        // ── Unused internal functions ────────────────────────────
        for name in &info.defined_values {
            let is_used_internally = info.used_values.contains(name);
            let is_exported =
                info.exposing.exposes_all || info.exposing.exposed_values.contains(name);

            if !is_used_internally && !is_exported {
                findings.push(Finding {
                    module_name: mod_name.clone(),
                    kind: FindingKind::UnusedFunction,
                    name: name.clone(),
                });
            }
        }

        // ── Unused exports ──────────────────────────────────────
        if !info.exposing.exposes_all {
            for name in &info.exposing.exposed_values {
                // Check if any other module imports this.
                let externally_used = imported_from
                    .get(&mod_name)
                    .is_some_and(|names| names.contains(name));
                let used_internally = info.used_values.contains(name);

                if !externally_used && !used_internally {
                    findings.push(Finding {
                        module_name: mod_name.clone(),
                        kind: FindingKind::UnusedExport,
                        name: name.clone(),
                    });
                }
            }
        }

        // ── Unused constructors ──────────────────────────────────
        for ctor_name in info.defined_constructors.keys() {
            let is_used = info.used_constructors.contains(ctor_name)
                || modules.values().any(|other| {
                    other.used_constructors.contains(ctor_name)
                        || other.used_qualified.iter().any(|(_, n)| n == ctor_name)
                });

            if !is_used {
                findings.push(Finding {
                    module_name: mod_name.clone(),
                    kind: FindingKind::UnusedConstructor,
                    name: ctor_name.clone(),
                });
            }
        }

        // ── Unused types ─────────────────────────────────────────
        for type_name in &info.defined_types {
            let is_used_internally = info.used_types.contains(type_name);
            let is_exported =
                info.exposing.exposes_all || info.exposing.exposed_types.contains(type_name);
            let is_used_externally = modules.values().any(|other| {
                other.used_types.contains(type_name)
                    || other
                        .used_qualified_types
                        .iter()
                        .any(|(_, n)| n == type_name)
            });

            if !is_used_internally && !is_exported && !is_used_externally {
                findings.push(Finding {
                    module_name: mod_name.clone(),
                    kind: FindingKind::UnusedType,
                    name: type_name.clone(),
                });
            }
        }
    }

    // Sort findings for stable output.
    findings.sort_by(|a, b| {
        a.module_name
            .cmp(&b.module_name)
            .then(a.kind.label().cmp(b.kind.label()))
            .then(a.name.cmp(&b.name))
    });

    findings
}

fn get_imported_names(imp: &crate::collect::ImportInfo) -> Vec<String> {
    match &imp.exposing {
        ImportExposingInfo::None => Vec::new(),
        ImportExposingInfo::All => Vec::new(), // can't enumerate
        ImportExposingInfo::Explicit(names) => names.clone(),
    }
}
