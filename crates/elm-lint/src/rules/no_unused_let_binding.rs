use std::collections::HashSet;

use elm_ast::expr::{Expr, LetDeclaration};
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

use crate::fix::remove_line;
use crate::rule::{Edit, Fix, LintContext, LintError, Rule, Severity};

/// Reports let bindings that are never referenced in the body.
pub struct NoUnusedLetBinding;

impl Rule for NoUnusedLetBinding {
    fn name(&self) -> &'static str {
        "NoUnusedLetBinding"
    }

    fn description(&self) -> &'static str {
        "Let binding is never used in the body"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut visitor = LetVisitor {
            source: ctx.source,
            errors: Vec::new(),
        };
        visitor.visit_module(ctx.module);
        visitor.errors
    }
}

struct LetVisitor<'a> {
    source: &'a str,
    errors: Vec<LintError>,
}

/// Collects all referenced names within an expression.
struct RefCollector {
    refs: HashSet<String>,
}

impl Visit for RefCollector {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::FunctionOrValue { module_name, name } = &expr.value {
            if module_name.is_empty() {
                self.refs.insert(name.clone());
            }
        }
        visit::walk_expr(self, expr);
    }
}

fn collect_refs(expr: &Spanned<Expr>) -> HashSet<String> {
    let mut collector = RefCollector {
        refs: HashSet::new(),
    };
    collector.visit_expr(expr);
    collector.refs
}

impl Visit for LetVisitor<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::LetIn {
            declarations, body, ..
        } = &expr.value
        {
            // Collect all names referenced in the body
            let body_refs = collect_refs(body);

            for decl in declarations {
                let (name, span) = match &decl.value {
                    LetDeclaration::Function(func) => {
                        let imp = &func.declaration.value;
                        (imp.name.value.clone(), decl.span)
                    }
                    LetDeclaration::Destructuring { .. } => {
                        // Skip destructuring — checking which pattern vars are
                        // unused is more complex and a separate concern.
                        continue;
                    }
                };

                if !body_refs.contains(&name) {
                    // Also check if it's referenced by other let declarations
                    let mut used_in_other_decls = false;
                    for other_decl in declarations {
                        if std::ptr::eq(
                            other_decl as *const Spanned<LetDeclaration>,
                            decl as *const Spanned<LetDeclaration>,
                        ) {
                            continue;
                        }
                        let other_refs = match &other_decl.value {
                            LetDeclaration::Function(func) => {
                                collect_refs(&func.declaration.value.body)
                            }
                            LetDeclaration::Destructuring { body, .. } => collect_refs(body),
                        };
                        if other_refs.contains(&name) {
                            used_in_other_decls = true;
                            break;
                        }
                    }

                    if !used_in_other_decls {
                        let fix = if declarations.len() == 1 {
                            // Only binding — replace entire let-in with body.
                            let body_text =
                                &self.source[body.span.start.offset..body.span.end.offset];
                            Some(Fix::replace(expr.span, body_text.to_string()))
                        } else {
                            // Multiple bindings — remove just this declaration.
                            let remove = Edit::Remove { span };
                            Some(Fix {
                                edits: vec![remove_line(self.source, &remove)],
                            })
                        };

                        self.errors.push(LintError {
                            rule: "NoUnusedLetBinding",
                            severity: Severity::Warning,
                            message: format!("Let binding `{name}` is never used"),
                            span,
                            fix,
                        });
                    }
                }
            }
        }
        visit::walk_expr(self, expr);
    }
}
