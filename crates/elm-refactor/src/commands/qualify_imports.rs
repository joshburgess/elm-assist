use elm_ast::exposing::{ExposedItem, Exposing};
use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::visit_mut::{self, VisitMut};

use crate::project::Project;

/// Convert unqualified imported names to qualified form and remove them
/// from the exposing list.
///
/// `import Html exposing (div, text)` + usage of `div` → `Html.div`
/// and removes `div` from the exposing list.
///
/// Does NOT qualify: types, constructors, or `exposing (..)`.
pub fn qualify_imports(project: &mut Project) -> usize {
    let mut total_changes = 0;

    for file in &mut project.files {
        // Collect which names are exposed from which modules.
        let mut exposed_names: Vec<(String, Vec<String>)> = Vec::new();

        for imp in &file.module.imports {
            let mod_name = imp.value.module_name.value.join(".");
            let alias = imp
                .value
                .alias
                .as_ref()
                .map(|a| a.value.join("."))
                .unwrap_or_else(|| mod_name.clone());

            if let Some(exp) = &imp.value.exposing {
                if let Exposing::Explicit(items) = &exp.value {
                    let fn_names: Vec<String> = items
                        .iter()
                        .filter_map(|item| match &item.value {
                            ExposedItem::Function(name) => Some(name.clone()),
                            _ => None,
                        })
                        .collect();
                    if !fn_names.is_empty() {
                        exposed_names.push((alias, fn_names));
                    }
                }
            }
        }

        // Qualify each exposed function name.
        for (module_alias, names) in &exposed_names {
            for name in names {
                let mut qualifier = Qualifier {
                    module_alias: module_alias.clone(),
                    name: name.clone(),
                    changes: 0,
                };
                qualifier.visit_module_mut(&mut file.module);
                total_changes += qualifier.changes;
            }
        }

        // Remove qualified names from import exposing lists.
        for imp in &mut file.module.imports {
            if let Some(exp) = &mut imp.value.exposing {
                if let Exposing::Explicit(items) = &mut exp.value {
                    let mod_name = imp.value.module_name.value.join(".");
                    let alias = imp
                        .value
                        .alias
                        .as_ref()
                        .map(|a| a.value.join("."))
                        .unwrap_or_else(|| mod_name.clone());

                    // Find which names from this import we qualified.
                    let qualified_names: Vec<String> = exposed_names
                        .iter()
                        .filter(|(a, _)| a == &alias)
                        .flat_map(|(_, names)| names.clone())
                        .collect();

                    items.retain(|item| {
                        if let ExposedItem::Function(name) = &item.value {
                            !qualified_names.contains(name)
                        } else {
                            true
                        }
                    });
                }
            }
        }

        // Remove empty exposing lists.
        for imp in &mut file.module.imports {
            if let Some(exp) = &imp.value.exposing {
                if let Exposing::Explicit(items) = &exp.value {
                    if items.is_empty() {
                        imp.value.exposing = None;
                    }
                }
            }
        }
    }

    total_changes
}

struct Qualifier {
    module_alias: String,
    name: String,
    changes: usize,
}

impl VisitMut for Qualifier {
    fn visit_expr_mut(&mut self, expr: &mut Spanned<Expr>) {
        if let Expr::FunctionOrValue { module_name, name } = &mut expr.value {
            if module_name.is_empty() && *name == self.name {
                *module_name = self
                    .module_alias
                    .split('.')
                    .map(|s| s.to_string())
                    .collect();
                self.changes += 1;
            }
        }
        visit_mut::walk_expr_mut(self, expr);
    }
}
