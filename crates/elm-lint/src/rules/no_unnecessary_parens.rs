use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports unnecessary parentheses around simple expressions.
pub struct NoUnnecessaryParens;

impl Rule for NoUnnecessaryParens {
    fn name(&self) -> &'static str {
        "NoUnnecessaryParens"
    }

    fn description(&self) -> &'static str {
        "Reports parentheses around expressions that don't need them"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut visitor = ParenVisitor {
            source: ctx.source,
            errors: Vec::new(),
        };
        visitor.visit_module(ctx.module);
        visitor.errors
    }
}

struct ParenVisitor<'a> {
    source: &'a str,
    errors: Vec<LintError>,
}

impl Visit for ParenVisitor<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::Parenthesized(inner) = &expr.value {
            // Parens around simple atoms are unnecessary.
            let is_simple = matches!(
                &inner.value,
                Expr::Unit
                    | Expr::Literal(_)
                    | Expr::FunctionOrValue { .. }
                    | Expr::RecordAccessFunction(_)
            );
            if is_simple {
                let inner_text = &self.source[inner.span.start.offset..inner.span.end.offset];
                self.errors.push(LintError {
                    rule: "NoUnnecessaryParens",
                    severity: Severity::Warning,
                    message: "Unnecessary parentheses".into(),
                    span: expr.span,
                    fix: Some(Fix::replace(expr.span, inner_text.to_string())),
                });
            }
        }
        visit::walk_expr(self, expr);
    }
}
