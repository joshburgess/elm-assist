use elm_ast::declaration::Declaration;
use elm_ast::file::ElmModule;
use elm_ast::span::Span;

use crate::rule::{LintContext, LintError, Rule, Severity};

pub struct NoUnusedCustomTypeConstructors;

impl Rule for NoUnusedCustomTypeConstructors {
    fn name(&self) -> &'static str {
        "NoUnusedCustomTypeConstructors"
    }

    fn description(&self) -> &'static str {
        "Custom type constructors that are never used in any module"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let (Some(info), Some(project)) = (ctx.module_info, ctx.project) else {
            return Vec::new();
        };

        let mut errors = Vec::new();

        for ctor_name in info.defined_constructors.keys() {
            let is_used = project.modules.values().any(|other| {
                other.used_constructors.contains(ctor_name)
                    || other.used_qualified.iter().any(|(_, n)| n == ctor_name)
            });

            if !is_used {
                let span =
                    find_constructor_span(ctx.module, ctor_name).unwrap_or(ctx.module.header.span);
                errors.push(LintError {
                    rule: "NoUnusedCustomTypeConstructors",
                    severity: Severity::Warning,
                    message: format!("Constructor `{ctor_name}` is never used"),
                    span,
                    fix: None,
                });
            }
        }

        errors
    }
}

fn find_constructor_span(module: &ElmModule, ctor_name: &str) -> Option<Span> {
    for decl in &module.declarations {
        if let Declaration::CustomTypeDeclaration(ct) = &decl.value {
            for ctor in &ct.constructors {
                if ctor.value.name.value == ctor_name {
                    return Some(ctor.value.name.span);
                }
            }
        }
    }
    None
}
