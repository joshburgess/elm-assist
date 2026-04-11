use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::pattern::Pattern;
use elm_ast::visit::{self, Visit};

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports `\x -> x` → `identity`.
pub struct NoIdentityFunction;

impl Rule for NoIdentityFunction {
    fn name(&self) -> &'static str {
        "NoIdentityFunction"
    }

    fn description(&self) -> &'static str {
        "Lambda that returns its only argument is equivalent to `identity`"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut visitor = Visitor { errors: Vec::new() };
        visitor.visit_module(ctx.module);
        visitor.errors
    }
}

struct Visitor {
    errors: Vec<LintError>,
}

impl Visit for Visitor {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::Lambda { args, body } = &expr.value {
            if args.len() == 1 {
                if let Pattern::Var(param_name) = &args[0].value {
                    if let Expr::FunctionOrValue { module_name, name } = &body.value {
                        if module_name.is_empty() && name == param_name {
                            self.errors.push(LintError {
                                rule: "NoIdentityFunction",
                                severity: Severity::Warning,
                                message: "`\\x -> x` is equivalent to `identity`".into(),
                                span: expr.span,
                                fix: Some(Fix::replace(expr.span, "identity".into())),
                            });
                        }
                    }
                }
            }
        }
        visit::walk_expr(self, expr);
    }
}
