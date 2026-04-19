use elm_ast::exposing::{ExposedItem, Exposing};
use elm_ast::node::Spanned;
use elm_ast::visit_mut::{self, VisitMut};

use crate::project::Project;

/// Rename a function/value across the entire project.
///
/// Renames the definition, all local references, all qualified references
/// from other modules, and updates exposing/import lists.
pub fn rename(project: &mut Project, module: &str, from: &str, to: &str) -> usize {
    let mut changes = 0;

    for file in &mut project.files {
        let is_defining_module = file.module_name == module;

        if is_defining_module {
            // Rename the definition itself + all local references.
            let mut renamer = LocalRenamer {
                from: from.to_string(),
                to: to.to_string(),
                changes: 0,
            };
            renamer.visit_module_mut(&mut file.module);
            changes += renamer.changes;

            // Update the exposing list.
            changes += rename_in_exposing(&mut file.module, from, to);
        } else {
            // In other modules, rename qualified references.
            let mut renamer = QualifiedRenamer {
                module_name: module.to_string(),
                from: from.to_string(),
                to: to.to_string(),
                changes: 0,
            };
            renamer.visit_module_mut(&mut file.module);
            changes += renamer.changes;

            // Check if this module imports the name via exposing.
            let imports_exposed = file.module.imports.iter().any(|imp| {
                imp.value.module_name.value.join(".") == module
                    && imp
                        .value
                        .exposing
                        .as_ref()
                        .is_some_and(|exp| import_exposes_name(&exp.value, from))
            });

            // If the name was exposed, also rename unqualified references.
            if imports_exposed {
                let mut local_renamer = LocalRenamer {
                    from: from.to_string(),
                    to: to.to_string(),
                    changes: 0,
                };
                local_renamer.visit_module_mut(&mut file.module);
                changes += local_renamer.changes;
            }

            // Update import exposing lists.
            for imp in &mut file.module.imports {
                if imp.value.module_name.value.join(".") == module {
                    if let Some(exp) = &mut imp.value.exposing {
                        changes += rename_in_import_exposing(&mut exp.value, from, to);
                    }
                }
            }
        }
    }

    changes
}

struct LocalRenamer {
    from: String,
    to: String,
    changes: usize,
}

impl VisitMut for LocalRenamer {
    fn visit_ident_mut(&mut self, name: &mut String) {
        if *name == self.from {
            *name = self.to.clone();
            self.changes += 1;
        }
    }
}

struct QualifiedRenamer {
    module_name: String,
    from: String,
    to: String,
    changes: usize,
}

impl VisitMut for QualifiedRenamer {
    fn visit_expr_mut(&mut self, expr: &mut Spanned<elm_ast::expr::Expr>) {
        if let elm_ast::expr::Expr::FunctionOrValue { module_name, name } = &mut expr.value {
            let qualified_module = module_name.join(".");
            if qualified_module == self.module_name && *name == self.from {
                *name = self.to.clone();
                self.changes += 1;
            }
        }
        visit_mut::walk_expr_mut(self, expr);
    }
}

fn rename_in_exposing(module: &mut elm_ast::file::ElmModule, from: &str, to: &str) -> usize {
    let exposing = match &mut module.header.value {
        elm_ast::module_header::ModuleHeader::Normal { exposing, .. }
        | elm_ast::module_header::ModuleHeader::Port { exposing, .. }
        | elm_ast::module_header::ModuleHeader::Effect { exposing, .. } => exposing,
    };

    rename_in_import_exposing(&mut exposing.value, from, to)
}

fn import_exposes_name(exposing: &Exposing, name: &str) -> bool {
    match exposing {
        Exposing::All(_) => true,
        Exposing::Explicit { items, .. } => items.iter().any(|item| match &item.value {
            ExposedItem::Function(n) => n == name,
            _ => false,
        }),
    }
}

fn rename_in_import_exposing(exposing: &mut Exposing, from: &str, to: &str) -> usize {
    let mut changes = 0;
    if let Exposing::Explicit { items, .. } = exposing {
        for item in items {
            match &mut item.value {
                ExposedItem::Function(name) if name.as_str() == from => {
                    *name = to.to_string();
                    changes += 1;
                }
                _ => {}
            }
        }
    }
    changes
}
