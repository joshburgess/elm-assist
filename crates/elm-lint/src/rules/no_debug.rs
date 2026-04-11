use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports uses of `Debug.log`, `Debug.todo`, and `Debug.toString`.
pub struct NoDebug;

impl Rule for NoDebug {
    fn name(&self) -> &'static str {
        "NoDebug"
    }

    fn description(&self) -> &'static str {
        "Disallows Debug.log, Debug.todo, and Debug.toString"
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut visitor = DebugVisitor(Vec::new());
        visitor.visit_module(ctx.module);
        visitor
            .0
            .into_iter()
            .map(|(span, name)| LintError {
                rule: self.name(),
                    severity: Severity::Warning,
                message: format!("`Debug.{name}` should not be used in production code"),
                span,
                fix: None,
            })
            .collect()
    }
}

struct DebugVisitor(Vec<(elm_ast::span::Span, String)>);

impl Visit for DebugVisitor {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::FunctionOrValue { module_name, name } = &expr.value {
            if module_name.len() == 1
                && module_name[0] == "Debug"
                && (name == "log" || name == "todo" || name == "toString")
            {
                self.0.push((expr.span, name.clone()));
            }
        }
        visit::walk_expr(self, expr);
    }
}
