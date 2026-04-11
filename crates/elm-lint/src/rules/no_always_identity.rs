use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports `always identity` which is just `identity`.
/// Also reports `identity >> f` or `f >> identity` which is just `f`,
/// and `always x` in places where a simple value would suffice.
pub struct NoAlwaysIdentity;

impl Rule for NoAlwaysIdentity {
    fn name(&self) -> &'static str {
        "NoAlwaysIdentity"
    }

    fn description(&self) -> &'static str {
        "Simplify `always identity` and identity composition"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut visitor = AlwaysVisitor {
            source: ctx.source,
            errors: Vec::new(),
        };
        visitor.visit_module(ctx.module);
        visitor.errors
    }
}

struct AlwaysVisitor<'a> {
    source: &'a str,
    errors: Vec<LintError>,
}

impl Visit for AlwaysVisitor<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        // `always identity` → `identity`
        if let Expr::Application(args) = &expr.value {
            if args.len() == 2
                && is_name(&args[0].value, "always")
                && is_name(&args[1].value, "identity")
            {
                self.errors.push(LintError {
                    rule: "NoAlwaysIdentity",
                    severity: Severity::Warning,
                    message: "`always identity` is equivalent to `identity`".into(),
                    span: expr.span,
                    fix: Some(Fix::replace(expr.span, "identity".into())),
                });
            }
        }

        // `identity >> f` or `f >> identity` → `f`
        // `identity << f` or `f << identity` → `f`
        if let Expr::OperatorApplication {
            operator,
            left,
            right,
            ..
        } = &expr.value
        {
            if operator == ">>" || operator == "<<" {
                let left_is_identity = is_name(&left.value, "identity");
                let right_is_identity = is_name(&right.value, "identity");

                if left_is_identity || right_is_identity {
                    // The non-identity side is the replacement.
                    let other = if left_is_identity { right } else { left };
                    let other_text = &self.source[other.span.start.offset..other.span.end.offset];

                    self.errors.push(LintError {
                        rule: "NoAlwaysIdentity",
                        severity: Severity::Warning,
                        message: format!(
                            "Composing with `identity` using `{operator}` has no effect"
                        ),
                        span: expr.span,
                        fix: Some(Fix::replace(expr.span, other_text.to_string())),
                    });
                }
            }
        }

        visit::walk_expr(self, expr);
    }
}

fn is_name(expr: &Expr, name: &str) -> bool {
    matches!(expr, Expr::FunctionOrValue { module_name, name: n } if module_name.is_empty() && n == name)
}
