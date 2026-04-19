use elm_ast::declaration::Declaration;
use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports usage of functions or values marked as deprecated via doc comments.
pub struct NoDeprecated;

impl Rule for NoDeprecated {
    fn name(&self) -> &'static str {
        "NoDeprecated"
    }

    fn description(&self) -> &'static str {
        "Do not use functions or values marked as @deprecated"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        // First, collect all names declared as deprecated in this module.
        let mut deprecated_names = std::collections::HashSet::new();

        for decl in &ctx.module.declarations {
            match &decl.value {
                Declaration::FunctionDeclaration(func)
                    if is_deprecated_doc(&func.documentation) =>
                {
                    deprecated_names.insert(func.declaration.value.name.value.clone());
                }
                Declaration::AliasDeclaration(alias)
                    if is_deprecated_doc(&alias.documentation) =>
                {
                    deprecated_names.insert(alias.name.value.clone());
                }
                Declaration::CustomTypeDeclaration(ct) if is_deprecated_doc(&ct.documentation) => {
                    deprecated_names.insert(ct.name.value.clone());
                }
                _ => {}
            }
        }

        if deprecated_names.is_empty() {
            return Vec::new();
        }

        let mut visitor = Visitor {
            deprecated_names: &deprecated_names,
            errors: Vec::new(),
        };
        visitor.visit_module(ctx.module);
        visitor.errors
    }
}

fn is_deprecated_doc(doc: &Option<Spanned<String>>) -> bool {
    match doc {
        Some(d) => {
            let lower = d.value.to_lowercase();
            lower.contains("@deprecated") || lower.contains("deprecated")
        }
        None => false,
    }
}

struct Visitor<'a> {
    deprecated_names: &'a std::collections::HashSet<String>,
    errors: Vec<LintError>,
}

impl Visit for Visitor<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::FunctionOrValue { module_name, name } = &expr.value {
            if module_name.is_empty() && self.deprecated_names.contains(name) {
                self.errors.push(LintError {
                    rule: "NoDeprecated",
                    severity: Severity::Warning,
                    message: format!("`{name}` is deprecated"),
                    span: expr.span,
                    fix: None,
                });
            }
        }
        visit::walk_expr(self, expr);
    }
}
