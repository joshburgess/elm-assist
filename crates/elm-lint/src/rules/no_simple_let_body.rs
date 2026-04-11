use elm_ast::expr::{Expr, LetDeclaration};
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports `let x = expr in x` → `expr` when the let has a single simple binding.
pub struct NoSimpleLetBody;

impl Rule for NoSimpleLetBody {
    fn name(&self) -> &'static str {
        "NoSimpleLetBody"
    }

    fn description(&self) -> &'static str {
        "Let-in that immediately returns its only binding can be simplified"
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
        if let Expr::LetIn { declarations, body } = &expr.value {
            if declarations.len() == 1 {
                if let LetDeclaration::Function(func) = &declarations[0].value {
                    let imp = &func.declaration.value;
                    // Only for simple bindings (no arguments)
                    if imp.args.is_empty() {
                        let binding_name = &imp.name.value;
                        // Body is just a reference to the binding name
                        if let Expr::FunctionOrValue { module_name, name } = &body.value {
                            if module_name.is_empty() && name == binding_name {
                                let rhs_text = &self.source
                                    [imp.body.span.start.offset..imp.body.span.end.offset];
                                self.errors.push(LintError {
                                    rule: "NoSimpleLetBody",
                    severity: Severity::Warning,
                                    message: format!(
                                        "`let {binding_name} = ... in {binding_name}` can be simplified to the binding's body"
                                    ),
                                    span: expr.span,
                                    fix: Some(Fix::replace(expr.span, rhs_text.to_string())),
                                });
                            }
                        }
                    }
                }
            }
        }
        visit::walk_expr(self, expr);
    }
}
