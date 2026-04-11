use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports `x :: []` which should be `[ x ]`.
pub struct NoRedundantCons;

impl Rule for NoRedundantCons {
    fn name(&self) -> &'static str {
        "NoRedundantCons"
    }

    fn description(&self) -> &'static str {
        "`x :: []` should be written as `[ x ]`"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut visitor = Visitor {
            source: ctx.source,
            errors: Vec::new(),
        };
        visitor.visit_module(ctx.module);
        visitor.errors
    }
}

struct Visitor<'a> {
    source: &'a str,
    errors: Vec<LintError>,
}

impl Visit for Visitor<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::OperatorApplication {
            operator,
            left,
            right,
            ..
        } = &expr.value
        {
            if operator == "::" {
                if let Expr::List(elems) = &right.value {
                    if elems.is_empty() {
                        let left_text = &self.source[left.span.start.offset..left.span.end.offset];
                        self.errors.push(LintError {
                            rule: "NoRedundantCons",
                            severity: Severity::Warning,
                            message: "`x :: []` can be simplified to `[ x ]`".into(),
                            span: expr.span,
                            fix: Some(Fix::replace(expr.span, format!("[ {left_text} ]"))),
                        });
                    }
                }
            }
        }
        visit::walk_expr(self, expr);
    }
}
