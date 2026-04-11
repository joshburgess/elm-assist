use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports double negation like `not (not x)` or `-(-(x))`.
pub struct NoNestedNegation;

impl Rule for NoNestedNegation {
    fn name(&self) -> &'static str {
        "NoNestedNegation"
    }

    fn description(&self) -> &'static str {
        "Double negation is redundant"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut visitor = NegVisitor {
            source: ctx.source,
            errors: Vec::new(),
        };
        visitor.visit_module(ctx.module);
        visitor.errors
    }
}

struct NegVisitor<'a> {
    source: &'a str,
    errors: Vec<LintError>,
}

impl Visit for NegVisitor<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        // `-(-(x))` → `x`
        if let Expr::Negation(inner) = &expr.value {
            if let Expr::Negation(innermost) = &inner.value {
                let inner_text =
                    &self.source[innermost.span.start.offset..innermost.span.end.offset];
                self.errors.push(LintError {
                    rule: "NoNestedNegation",
                    severity: Severity::Warning,
                    message: "Double negation — simplify by removing both".into(),
                    span: expr.span,
                    fix: Some(Fix::replace(expr.span, inner_text.to_string())),
                });
            }
        }

        // `not (not x)` → `x`
        if let Expr::Application(args) = &expr.value {
            if args.len() == 2 {
                if let Expr::FunctionOrValue { module_name, name } = &args[0].value {
                    if module_name.is_empty() && name == "not" {
                        if let Expr::Parenthesized(inner) = &args[1].value {
                            if let Expr::Application(inner_args) = &inner.value {
                                if inner_args.len() == 2 {
                                    if let Expr::FunctionOrValue {
                                        module_name: m2,
                                        name: n2,
                                    } = &inner_args[0].value
                                    {
                                        if m2.is_empty() && n2 == "not" {
                                            let arg = &inner_args[1];
                                            let arg_text = &self.source
                                                [arg.span.start.offset..arg.span.end.offset];
                                            self.errors.push(LintError {
                                                rule: "NoNestedNegation",
                    severity: Severity::Warning,
                                                message: "Double negation — simplify by removing both".into(),
                                                span: expr.span,
                                                fix: Some(Fix::replace(expr.span, arg_text.to_string())),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        visit::walk_expr(self, expr);
    }
}
