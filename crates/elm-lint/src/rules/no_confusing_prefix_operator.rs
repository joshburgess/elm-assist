use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports confusing uses of non-commutative operators in prefix form.
///
/// `(-) a b` is confusing because it's unclear whether it means `a - b` or `b - a`.
/// (It means `a - b`, but many developers get this wrong.)
///
/// Commutative operators like `(+)` and `(*)` are fine in prefix form.
pub struct NoConfusingPrefixOperator;

impl Rule for NoConfusingPrefixOperator {
    fn name(&self) -> &'static str {
        "NoConfusingPrefixOperator"
    }

    fn description(&self) -> &'static str {
        "Non-commutative operators should not be used in prefix form"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut visitor = Visitor {
            errors: Vec::new(),
        };
        visitor.visit_module(ctx.module);
        visitor.errors
    }
}

const NON_COMMUTATIVE_OPS: &[&str] = &[
    "-", "/", "//", "^", "::", "++", "<|", "|>", ">>", "<<",
    "<", ">", "<=", ">=", "/=",
];

struct Visitor {
    errors: Vec<LintError>,
}

impl Visit for Visitor {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        // Look for Application where the first element is a PrefixOperator
        if let Expr::Application(args) = &expr.value {
            if args.len() >= 2 {
                if let Expr::PrefixOperator(op) = &args[0].value {
                    if NON_COMMUTATIVE_OPS.contains(&op.as_str()) {
                        self.errors.push(LintError {
                            rule: "NoConfusingPrefixOperator",
                            severity: Severity::Warning,
                            message: format!(
                                "`({op})` is a non-commutative operator used in prefix form — this can be confusing"
                            ),
                            span: args[0].span,
                            fix: None,
                        });
                    }
                }
            }
        }
        visit::walk_expr(self, expr);
    }
}
