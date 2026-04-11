use elm_ast::exposing::{ExposedItem, Exposing};
use elm_ast::file::ElmModule;
use elm_ast::module_header::ModuleHeader;
use elm_ast::span::Span;

use crate::rule::{LintContext, LintError, Rule, Severity};

pub struct NoUnusedExports;

impl Rule for NoUnusedExports {
    fn name(&self) -> &'static str {
        "NoUnusedExports"
    }

    fn description(&self) -> &'static str {
        "Exported values/types that no other module imports"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let (Some(info), Some(project)) = (ctx.module_info, ctx.project) else {
            return Vec::new();
        };

        // Skip modules that expose everything — can't pinpoint individual unused exports.
        if info.exposing.exposes_all {
            return Vec::new();
        }

        let mod_name = info.module_name.join(".");
        let mut errors = Vec::new();

        // Check exported values/functions.
        for name in &info.exposing.exposed_values {
            let externally_used = project
                .imported_from
                .get(&mod_name)
                .is_some_and(|names| names.contains(name));
            let used_internally = info.used_values.contains(name);

            if !externally_used && !used_internally {
                let span = find_exposed_item_span(ctx.module, name)
                    .unwrap_or(ctx.module.header.span);
                errors.push(LintError {
                    rule: "NoUnusedExports",
                    severity: Severity::Warning,
                    message: format!("`{name}` is exported but never imported by any module"),
                    span,
                    fix: None,
                });
            }
        }

        // Check exported types.
        for name in &info.exposing.exposed_types {
            let externally_used = project
                .imported_from
                .get(&mod_name)
                .is_some_and(|names| names.contains(name));
            let used_internally = info.used_types.contains(name);

            if !externally_used && !used_internally {
                let span = find_exposed_item_span(ctx.module, name)
                    .unwrap_or(ctx.module.header.span);
                errors.push(LintError {
                    rule: "NoUnusedExports",
                    severity: Severity::Warning,
                    message: format!("Type `{name}` is exported but never imported by any module"),
                    span,
                    fix: None,
                });
            }
        }

        errors
    }
}

fn find_exposed_item_span(module: &ElmModule, name: &str) -> Option<Span> {
    let exposing_node = match &module.header.value {
        ModuleHeader::Normal { exposing, .. }
        | ModuleHeader::Port { exposing, .. }
        | ModuleHeader::Effect { exposing, .. } => exposing,
    };
    if let Exposing::Explicit(items) = &exposing_node.value {
        for item in items {
            let item_name = match &item.value {
                ExposedItem::Function(n) => n.as_str(),
                ExposedItem::TypeOrAlias(n) => n.as_str(),
                ExposedItem::TypeExpose { name, .. } => name.as_str(),
                ExposedItem::Infix(n) => n.as_str(),
            };
            if item_name == name {
                return Some(item.span);
            }
        }
    }
    None
}
