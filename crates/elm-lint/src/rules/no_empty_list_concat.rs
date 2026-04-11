use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports `[] ++ list` → `list` and `list ++ []` → `list`.
pub struct NoEmptyListConcat;

impl Rule for NoEmptyListConcat {
    fn name(&self) -> &'static str {
        "NoEmptyListConcat"
    }

    fn description(&self) -> &'static str {
        "Concatenating with an empty list has no effect"
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

fn is_empty_list(expr: &Expr) -> bool {
    matches!(expr, Expr::List(elems) if elems.is_empty())
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
            if operator == "++" {
                if is_empty_list(&left.value) {
                    let right_text = &self.source[right.span.start.offset..right.span.end.offset];
                    self.errors.push(LintError {
                        rule: "NoEmptyListConcat",
                        severity: Severity::Warning,
                        message: "`[] ++ list` is equivalent to `list`".into(),
                        span: expr.span,
                        fix: Some(Fix::replace(expr.span, right_text.to_string())),
                    });
                } else if is_empty_list(&right.value) {
                    let left_text = &self.source[left.span.start.offset..left.span.end.offset];
                    self.errors.push(LintError {
                        rule: "NoEmptyListConcat",
                        severity: Severity::Warning,
                        message: "`list ++ []` is equivalent to `list`".into(),
                        span: expr.span,
                        fix: Some(Fix::replace(expr.span, left_text.to_string())),
                    });
                }
            }
        }
        visit::walk_expr(self, expr);
    }
}
