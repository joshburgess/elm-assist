use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports `[a] ++ [b]` → `[a, b]` when both sides are list literals.
pub struct NoListLiteralConcat;

impl Rule for NoListLiteralConcat {
    fn name(&self) -> &'static str {
        "NoListLiteralConcat"
    }

    fn description(&self) -> &'static str {
        "List literal concatenation can be written as a single list"
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
        if let Expr::OperatorApplication {
            operator,
            left,
            right,
            ..
        } = &expr.value
        {
            if operator == "++" {
                if let (Expr::List(left_elems), Expr::List(right_elems)) =
                    (&left.value, &right.value)
                {
                    // Both sides are list literals — merge them.
                    // Skip if both are empty (that's NoEmptyListConcat territory).
                    if left_elems.is_empty() && right_elems.is_empty() {
                        // Let NoEmptyListConcat handle this.
                    } else if !left_elems.is_empty() || !right_elems.is_empty() {
                        let all_elems: Vec<&str> = left_elems
                            .iter()
                            .chain(right_elems.iter())
                            .map(|e| &self.source[e.span.start.offset..e.span.end.offset])
                            .collect();
                        let merged = format!("[ {} ]", all_elems.join(", "));
                        self.errors.push(LintError {
                            rule: "NoListLiteralConcat",
                    severity: Severity::Warning,
                            message: "Two list literals concatenated can be written as one list"
                                .into(),
                            span: expr.span,
                            fix: Some(Fix::replace(expr.span, merged)),
                        });
                    }
                }
            }
        }
        visit::walk_expr(self, expr);
    }
}
