use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports `Result.map f (Err e)` → `Err e`.
pub struct NoResultMapWithErr;

impl Rule for NoResultMapWithErr {
    fn name(&self) -> &'static str {
        "NoResultMapWithErr"
    }

    fn description(&self) -> &'static str {
        "Mapping over an `Err` always returns the same `Err`"
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

fn is_result_map(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::FunctionOrValue { module_name, name }
            if name == "map" && module_name.len() == 1 && module_name[0] == "Result"
    )
}

fn is_err_application(expr: &Expr) -> bool {
    match expr {
        Expr::Application(args) if args.len() == 2 => {
            matches!(
                &args[0].value,
                Expr::FunctionOrValue { module_name, name }
                    if module_name.is_empty() && name == "Err"
            )
        }
        Expr::Parenthesized { expr: inner, .. } => is_err_application(&inner.value),
        _ => false,
    }
}

impl Visit for Visitor<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        // Result.map f (Err e) → Err e
        if let Expr::Application(args) = &expr.value {
            if args.len() == 3
                && is_result_map(&args[0].value)
                && is_err_application(&args[2].value)
            {
                let err_text = &self.source[args[2].span.start.offset..args[2].span.end.offset];
                self.errors.push(LintError {
                    rule: "NoResultMapWithErr",
                    severity: Severity::Warning,
                    message: "`Result.map f (Err e)` is always `Err e`".into(),
                    span: expr.span,
                    fix: Some(Fix::replace(expr.span, err_text.to_string())),
                });
            }
        }
        visit::walk_expr(self, expr);
    }
}
