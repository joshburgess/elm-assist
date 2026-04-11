use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports `{ record | }` with no actual updates, which is just `record`.
pub struct NoEmptyRecordUpdate;

impl Rule for NoEmptyRecordUpdate {
    fn name(&self) -> &'static str {
        "NoEmptyRecordUpdate"
    }

    fn description(&self) -> &'static str {
        "Record update with no fields is equivalent to the record itself"
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
        if let Expr::RecordUpdate { base, updates } = &expr.value {
            if updates.is_empty() {
                let name_text = &self.source[base.span.start.offset..base.span.end.offset];
                self.errors.push(LintError {
                    rule: "NoEmptyRecordUpdate",
                    severity: Severity::Warning,
                    message: "Record update with no fields — just use the record directly".into(),
                    span: expr.span,
                    fix: Some(Fix::replace(expr.span, name_text.to_string())),
                });
            }
        }
        visit::walk_expr(self, expr);
    }
}
