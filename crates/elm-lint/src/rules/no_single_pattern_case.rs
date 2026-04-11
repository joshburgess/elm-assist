use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports `case` expressions with only one branch (should use `let` instead).
pub struct NoSinglePatternCase;

impl Rule for NoSinglePatternCase {
    fn name(&self) -> &'static str {
        "NoSinglePatternCase"
    }

    fn description(&self) -> &'static str {
        "Case expressions with a single branch can be replaced with let destructuring"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut visitor = CaseVisitor(Vec::new());
        visitor.visit_module(ctx.module);
        visitor
            .0
            .into_iter()
            .map(|span| LintError {
                rule: self.name(),
                severity: Severity::Warning,
                message: "Case expression has only one branch — consider using `let` destructuring"
                    .into(),
                span,
                fix: None,
            })
            .collect()
    }
}

struct CaseVisitor(Vec<elm_ast::span::Span>);

impl Visit for CaseVisitor {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::CaseOf { branches, .. } = &expr.value {
            if branches.len() == 1 {
                self.0.push(expr.span);
            }
        }
        visit::walk_expr(self, expr);
    }
}
