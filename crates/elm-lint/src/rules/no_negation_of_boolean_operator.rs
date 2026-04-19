use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports `not (a == b)` → `a /= b`, `not (a < b)` → `a >= b`, etc.
pub struct NoNegationOfBooleanOperator;

impl Rule for NoNegationOfBooleanOperator {
    fn name(&self) -> &'static str {
        "NoNegationOfBooleanOperator"
    }

    fn description(&self) -> &'static str {
        "Use the negated operator instead of wrapping with `not`"
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

fn negate_operator(op: &str) -> Option<&'static str> {
    match op {
        "==" => Some("/="),
        "/=" => Some("=="),
        "<" => Some(">="),
        ">" => Some("<="),
        "<=" => Some(">"),
        ">=" => Some("<"),
        _ => None,
    }
}

fn is_not(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::FunctionOrValue { module_name, name }
            if module_name.is_empty() && name == "not"
    )
}

impl Visit for Visitor<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        // not (a == b) → a /= b
        // Pattern: Application [ not, Parenthesized(OperatorApplication { op, left, right }) ]
        if let Expr::Application(args) = &expr.value {
            if args.len() == 2 && is_not(&args[0].value) {
                if let Expr::Parenthesized { expr: inner, .. } = &args[1].value {
                    if let Expr::OperatorApplication {
                        operator,
                        left,
                        right,
                        ..
                    } = &inner.value
                    {
                        if let Some(negated) = negate_operator(operator) {
                            let left_text =
                                &self.source[left.span.start.offset..left.span.end.offset];
                            let right_text =
                                &self.source[right.span.start.offset..right.span.end.offset];
                            let replacement = format!("{left_text} {negated} {right_text}");
                            self.errors.push(LintError {
                                rule: "NoNegationOfBooleanOperator",
                                severity: Severity::Warning,
                                message: format!(
                                    "`not (a {operator} b)` can be written as `a {negated} b`"
                                ),
                                span: expr.span,
                                fix: Some(Fix::replace(expr.span, replacement)),
                            });
                        }
                    }
                }
            }
        }
        visit::walk_expr(self, expr);
    }
}
