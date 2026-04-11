use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::pattern::Pattern;
use elm_ast::visit::{self, Visit};

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports `case x of True -> ... ; False -> ...` which should be `if x then ... else ...`.
pub struct NoBooleanCase;

impl Rule for NoBooleanCase {
    fn name(&self) -> &'static str {
        "NoBooleanCase"
    }

    fn description(&self) -> &'static str {
        "Case on Bool should be replaced with if-then-else"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut visitor = BoolCaseVisitor(Vec::new());
        visitor.visit_module(ctx.module);
        visitor
            .0
            .into_iter()
            .map(|span| LintError {
                rule: self.name(),
                severity: Severity::Warning,
                message: "Use `if`/`else` instead of `case` on Bool".into(),
                span,
                fix: None,
            })
            .collect()
    }
}

struct BoolCaseVisitor(Vec<elm_ast::span::Span>);

impl Visit for BoolCaseVisitor {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::CaseOf { branches, .. } = &expr.value {
            if branches.len() == 2 {
                let pats: Vec<&str> = branches
                    .iter()
                    .filter_map(|b| match &b.pattern.value {
                        Pattern::Constructor {
                            module_name,
                            name,
                            args,
                            ..
                        } if args.is_empty() && module_name.is_empty() => Some(name.as_str()),
                        _ => None,
                    })
                    .collect();
                if pats.len() == 2
                    && ((pats[0] == "True" && pats[1] == "False")
                        || (pats[0] == "False" && pats[1] == "True"))
                {
                    self.0.push(expr.span);
                }
            }
        }
        visit::walk_expr(self, expr);
    }
}
