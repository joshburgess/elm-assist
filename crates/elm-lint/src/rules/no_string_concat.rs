use elm_ast::expr::Expr;
use elm_ast::literal::Literal;
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports `"a" ++ "b"` → `"ab"` when both sides are string literals.
pub struct NoStringConcat;

impl Rule for NoStringConcat {
    fn name(&self) -> &'static str {
        "NoStringConcat"
    }

    fn description(&self) -> &'static str {
        "String literal concatenation can be written as a single string"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut visitor = Visitor { errors: Vec::new() };
        visitor.visit_module(ctx.module);
        visitor.errors
    }
}

struct Visitor {
    errors: Vec<LintError>,
}

impl Visit for Visitor {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::OperatorApplication {
            operator,
            left,
            right,
            ..
        } = &expr.value
        {
            if operator == "++" {
                if let (Expr::Literal(Literal::String(l)), Expr::Literal(Literal::String(r))) =
                    (&left.value, &right.value)
                {
                    let merged = format!("\"{}{}\"", l, r);
                    self.errors.push(LintError {
                        rule: "NoStringConcat",
                        severity: Severity::Warning,
                        message: "Two string literals concatenated can be written as one".into(),
                        span: expr.span,
                        fix: Some(Fix::replace(expr.span, merged)),
                    });
                }
            }
        }
        visit::walk_expr(self, expr);
    }
}
