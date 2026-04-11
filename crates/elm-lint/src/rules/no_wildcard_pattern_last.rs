use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::pattern::Pattern;
use elm_ast::visit::{self, Visit};

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports `case` expressions where a wildcard `_` pattern is not the last branch.
pub struct NoWildcardPatternLast;

impl Rule for NoWildcardPatternLast {
    fn name(&self) -> &'static str {
        "NoWildcardPatternLast"
    }

    fn description(&self) -> &'static str {
        "Wildcard `_` pattern should only appear as the last case branch"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut visitor = Visitor(Vec::new());
        visitor.visit_module(ctx.module);
        visitor.0
    }
}

struct Visitor(Vec<LintError>);

impl Visit for Visitor {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::CaseOf { branches, .. } = &expr.value {
            if branches.len() > 1 {
                // Check all branches except the last.
                for branch in &branches[..branches.len() - 1] {
                    if is_wildcard(&branch.pattern.value) {
                        self.0.push(LintError {
                            rule: "NoWildcardPatternLast",
                    severity: Severity::Warning,
                            message:
                                "Wildcard `_` pattern is not the last branch — subsequent branches are unreachable"
                                    .into(),
                            span: branch.pattern.span,
                            fix: None,
                        });
                    }
                }
            }
        }
        visit::walk_expr(self, expr);
    }
}

fn is_wildcard(pat: &Pattern) -> bool {
    matches!(pat, Pattern::Anything | Pattern::Var(_))
}
