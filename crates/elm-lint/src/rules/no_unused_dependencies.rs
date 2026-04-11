use std::collections::HashSet;

use crate::elm_json::packages_used_by_imports;
use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports dependencies listed in elm.json that are never imported by any
/// module in the project. Requires elm.json to be discoverable.
///
/// Resolves package modules from the Elm package cache (~/.elm/0.19.1/packages/)
/// when available, with a hardcoded fallback for ~30 popular packages.
/// Unknown packages are silently skipped (no false positives).
pub struct NoUnusedDependencies;

impl Rule for NoUnusedDependencies {
    fn name(&self) -> &'static str {
        "NoUnusedDependencies"
    }

    fn description(&self) -> &'static str {
        "Dependencies in elm.json should be used by at least one import"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let (Some(project), Some(info)) = (ctx.project, ctx.module_info) else {
            return Vec::new();
        };
        let Some(elm_json) = &project.elm_json else {
            return Vec::new();
        };

        // This is a project-level rule — only report from one module to avoid
        // duplicate errors. We pick the alphabetically-first module.
        let my_name = info.module_name.join(".");
        let first_module = project.modules.keys().min().cloned().unwrap_or_default();
        if my_name != first_module {
            return Vec::new();
        }

        // Collect all imported module names across the entire project.
        let mut all_imported_modules: HashSet<String> = HashSet::new();
        for module_info in project.modules.values() {
            for imp in &module_info.imports {
                all_imported_modules.insert(imp.module_name.join("."));
            }
        }

        let used_packages =
            packages_used_by_imports(&all_imported_modules, &elm_json.package_modules);

        let mut errors = Vec::new();

        let mut dep_names: Vec<&String> = elm_json.direct_deps.keys().collect();
        dep_names.sort();

        for dep_name in dep_names {
            // Skip elm/core — it's always implicitly used.
            if dep_name == "elm/core" {
                continue;
            }

            // Only check packages we know the modules for.
            if !elm_json.package_modules.contains_key(dep_name) {
                continue;
            }

            if !used_packages.contains(dep_name.as_str()) {
                errors.push(LintError {
                    rule: "NoUnusedDependencies",
                    severity: Severity::Warning,
                    message: format!(
                        "Dependency `{dep_name}` is listed in elm.json but none of its modules are imported"
                    ),
                    span: ctx.module.header.span,
                    fix: None,
                });
            }
        }

        errors
    }
}
