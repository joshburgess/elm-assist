use elm_ast::declaration::Declaration;
use elm_ast::expr::{Expr, LetDeclaration};
use elm_ast::node::Spanned;

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports calling `update` within `update` — a common anti-pattern in
/// The Elm Architecture that leads to hard-to-follow control flow.
pub struct NoRecursiveUpdate;

impl Rule for NoRecursiveUpdate {
    fn name(&self) -> &'static str {
        "NoRecursiveUpdate"
    }

    fn description(&self) -> &'static str {
        "The update function should not call itself recursively"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        // Find the `update` function.
        for decl in &ctx.module.declarations {
            if let Declaration::FunctionDeclaration(func) = &decl.value {
                let name = &func.declaration.value.name.value;
                if name == "update" {
                    let mut visitor = Visitor {
                        errors: Vec::new(),
                    };
                    visitor.check_expr(&func.declaration.value.body);
                    return visitor.errors;
                }
            }
        }

        Vec::new()
    }
}

struct Visitor {
    errors: Vec<LintError>,
}

impl Visitor {
    fn check_expr(&mut self, expr: &Spanned<Expr>) {
        match &expr.value {
            Expr::Application(args) => {
                if let Some(first) = args.first() {
                    if let Expr::FunctionOrValue { module_name, name } = &first.value {
                        if module_name.is_empty() && name == "update" {
                            self.errors.push(LintError {
                                rule: "NoRecursiveUpdate",
                                severity: Severity::Warning,
                                message:
                                    "`update` should not call itself recursively — extract shared logic into a helper"
                                        .to_string(),
                                span: first.span,
                                fix: None,
                            });
                        }
                    }
                }
                for a in args {
                    self.check_expr(a);
                }
            }
            Expr::IfElse {
                branches,
                else_branch,
            } => {
                for (c, b) in branches {
                    self.check_expr(c);
                    self.check_expr(b);
                }
                self.check_expr(else_branch);
            }
            Expr::CaseOf {
                expr: subject,
                branches,
            } => {
                self.check_expr(subject);
                for b in branches {
                    self.check_expr(&b.body);
                }
            }
            Expr::LetIn { declarations, body } => {
                for d in declarations {
                    match &d.value {
                        LetDeclaration::Function(f) => {
                            self.check_expr(&f.declaration.value.body);
                        }
                        LetDeclaration::Destructuring { body: b, .. } => {
                            self.check_expr(b);
                        }
                    }
                }
                self.check_expr(body);
            }
            Expr::Lambda { body, .. } => self.check_expr(body),
            Expr::OperatorApplication { left, right, .. } => {
                self.check_expr(left);
                self.check_expr(right);
            }
            Expr::Parenthesized(inner) | Expr::Negation(inner) => {
                self.check_expr(inner);
            }
            Expr::Tuple(elems) | Expr::List(elems) => {
                for e in elems {
                    self.check_expr(e);
                }
            }
            Expr::Record(fields) => {
                for f in fields {
                    self.check_expr(&f.value.value);
                }
            }
            Expr::RecordUpdate { updates, .. } => {
                for f in updates {
                    self.check_expr(&f.value.value);
                }
            }
            Expr::RecordAccess { record, .. } => {
                self.check_expr(record);
            }
            _ => {}
        }
    }
}
