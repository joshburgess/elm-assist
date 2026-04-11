use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports `x |> identity` → `x` and `identity <| x` → `x`.
pub struct NoPipelineSimplify;

impl Rule for NoPipelineSimplify {
    fn name(&self) -> &'static str {
        "NoPipelineSimplify"
    }

    fn description(&self) -> &'static str {
        "Piping through `identity` has no effect"
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

fn is_identity(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::FunctionOrValue { module_name, name }
            if module_name.is_empty() && name == "identity"
    )
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
            // x |> identity → x
            if operator == "|>" && is_identity(&right.value) {
                let left_text = &self.source[left.span.start.offset..left.span.end.offset];
                self.errors.push(LintError {
                    rule: "NoPipelineSimplify",
                    severity: Severity::Warning,
                    message: "`x |> identity` is equivalent to `x`".into(),
                    span: expr.span,
                    fix: Some(Fix::replace(expr.span, left_text.to_string())),
                });
            }
            // identity <| x → x
            else if operator == "<|" && is_identity(&left.value) {
                let right_text =
                    &self.source[right.span.start.offset..right.span.end.offset];
                self.errors.push(LintError {
                    rule: "NoPipelineSimplify",
                    severity: Severity::Warning,
                    message: "`identity <| x` is equivalent to `x`".into(),
                    span: expr.span,
                    fix: Some(Fix::replace(expr.span, right_text.to_string())),
                });
            }
        }
        visit::walk_expr(self, expr);
    }
}
