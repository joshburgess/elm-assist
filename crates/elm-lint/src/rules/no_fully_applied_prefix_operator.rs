use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports `(+) 1 2` → `1 + 2` for fully applied prefix operators.
pub struct NoFullyAppliedPrefixOperator;

impl Rule for NoFullyAppliedPrefixOperator {
    fn name(&self) -> &'static str {
        "NoFullyAppliedPrefixOperator"
    }

    fn description(&self) -> &'static str {
        "Fully applied prefix operators can be written in infix form"
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
        // (+) a b → a + b
        // Pattern: Application [ PrefixOperator(op), a, b ]
        if let Expr::Application(args) = &expr.value {
            if args.len() == 3 {
                if let Expr::PrefixOperator(op) = &args[0].value {
                    let left_text =
                        &self.source[args[1].span.start.offset..args[1].span.end.offset];
                    let right_text =
                        &self.source[args[2].span.start.offset..args[2].span.end.offset];
                    let replacement = format!("{left_text} {op} {right_text}");
                    self.errors.push(LintError {
                        rule: "NoFullyAppliedPrefixOperator",
                    severity: Severity::Warning,
                        message: format!(
                            "`({op}) a b` can be written as `a {op} b`"
                        ),
                        span: expr.span,
                        fix: Some(Fix::replace(expr.span, replacement)),
                    });
                }
            }
        }
        visit::walk_expr(self, expr);
    }
}
