use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports `Maybe.map f Nothing` → `Nothing`.
pub struct NoMaybeMapWithNothing;

impl Rule for NoMaybeMapWithNothing {
    fn name(&self) -> &'static str {
        "NoMaybeMapWithNothing"
    }

    fn description(&self) -> &'static str {
        "Mapping over `Nothing` always returns `Nothing`"
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

fn is_maybe_map(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::FunctionOrValue { module_name, name }
            if name == "map" && module_name.len() == 1 && module_name[0] == "Maybe"
    )
}

fn is_nothing(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::FunctionOrValue { module_name, name }
            if module_name.is_empty() && name == "Nothing"
    )
}

impl Visit for Visitor {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        // Maybe.map f Nothing → Nothing
        if let Expr::Application(args) = &expr.value {
            if args.len() == 3 && is_maybe_map(&args[0].value) && is_nothing(&args[2].value) {
                self.errors.push(LintError {
                    rule: "NoMaybeMapWithNothing",
                    severity: Severity::Warning,
                    message: "`Maybe.map f Nothing` is always `Nothing`".into(),
                    span: expr.span,
                    fix: Some(Fix::replace(expr.span, "Nothing".into())),
                });
            }
        }
        visit::walk_expr(self, expr);
    }
}
