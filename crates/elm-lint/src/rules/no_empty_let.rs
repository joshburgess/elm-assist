use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports `let in` expressions with no declarations.
pub struct NoEmptyLet;

impl Rule for NoEmptyLet {
    fn name(&self) -> &'static str {
        "NoEmptyLet"
    }

    fn description(&self) -> &'static str {
        "Let expressions with no declarations are useless"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut visitor = LetVisitor {
            source: ctx.source,
            errors: Vec::new(),
        };
        visitor.visit_module(ctx.module);
        visitor.errors
    }
}

struct LetVisitor<'a> {
    source: &'a str,
    errors: Vec<LintError>,
}

impl Visit for LetVisitor<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::LetIn { declarations, body } = &expr.value {
            if declarations.is_empty() {
                let body_text =
                    &self.source[body.span.start.offset..body.span.end.offset];
                self.errors.push(LintError {
                    rule: "NoEmptyLet",
                    severity: Severity::Warning,
                    message: "Empty `let` expression — remove the `let ... in` wrapper".into(),
                    span: expr.span,
                    fix: Some(Fix::replace(expr.span, body_text.to_string())),
                });
            }
        }
        visit::walk_expr(self, expr);
    }
}
