use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Simplifies boolean operator expressions with literal operands.
/// `x && True` → `x`, `x || False` → `x`, `x && False` → `False`, `x || True` → `True`
pub struct NoBoolOperatorSimplify;

impl Rule for NoBoolOperatorSimplify {
    fn name(&self) -> &'static str {
        "NoBoolOperatorSimplify"
    }

    fn description(&self) -> &'static str {
        "Simplify boolean expressions with literal True/False operands"
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

fn is_bool(expr: &Expr) -> Option<bool> {
    match expr {
        Expr::FunctionOrValue { module_name, name } if module_name.is_empty() => {
            match name.as_str() {
                "True" => Some(true),
                "False" => Some(false),
                _ => None,
            }
        }
        _ => None,
    }
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
            let left_bool = is_bool(&left.value);
            let right_bool = is_bool(&right.value);

            let replacement = match (operator.as_str(), left_bool, right_bool) {
                // x && True → x
                ("&&", None, Some(true)) => {
                    Some(self.source[left.span.start.offset..left.span.end.offset].to_string())
                }
                // True && x → x
                ("&&", Some(true), None) => {
                    Some(self.source[right.span.start.offset..right.span.end.offset].to_string())
                }
                // x && False → False
                ("&&", None, Some(false)) => Some("False".into()),
                // False && x → False
                ("&&", Some(false), None) => Some("False".into()),
                // x || False → x
                ("||", None, Some(false)) => {
                    Some(self.source[left.span.start.offset..left.span.end.offset].to_string())
                }
                // False || x → x
                ("||", Some(false), None) => {
                    Some(self.source[right.span.start.offset..right.span.end.offset].to_string())
                }
                // x || True → True
                ("||", None, Some(true)) => Some("True".into()),
                // True || x → True
                ("||", Some(true), None) => Some("True".into()),
                _ => None,
            };

            if let Some(replacement) = replacement {
                self.errors.push(LintError {
                    rule: "NoBoolOperatorSimplify",
                    severity: Severity::Warning,
                    message: format!(
                        "Boolean expression with `{operator}` and a literal can be simplified"
                    ),
                    span: expr.span,
                    fix: Some(Fix::replace(expr.span, replacement)),
                });
            }
        }
        visit::walk_expr(self, expr);
    }
}
