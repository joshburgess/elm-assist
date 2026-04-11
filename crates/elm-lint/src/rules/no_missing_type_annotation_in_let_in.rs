use elm_ast::expr::{Expr, LetDeclaration};
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports `let..in` function declarations that lack type annotations.
pub struct NoMissingTypeAnnotationInLetIn;

impl Rule for NoMissingTypeAnnotationInLetIn {
    fn name(&self) -> &'static str {
        "NoMissingTypeAnnotationInLetIn"
    }

    fn description(&self) -> &'static str {
        "Let-in bindings should have type annotations"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut visitor = Visitor {
            errors: Vec::new(),
        };
        visitor.visit_module(ctx.module);
        visitor.errors
    }
}

struct Visitor {
    errors: Vec<LintError>,
}

impl Visit for Visitor {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::LetIn { declarations, .. } = &expr.value {
            for decl in declarations {
                if let LetDeclaration::Function(func) = &decl.value {
                    if func.signature.is_none() {
                        let name = &func.declaration.value.name.value;
                        self.errors.push(LintError {
                            rule: "NoMissingTypeAnnotationInLetIn",
                            severity: Severity::Warning,
                            message: format!(
                                "Let binding `{name}` is missing a type annotation"
                            ),
                            span: func.declaration.value.name.span,
                            fix: None,
                        });
                    }
                }
            }
        }
        visit::walk_expr(self, expr);
    }
}
