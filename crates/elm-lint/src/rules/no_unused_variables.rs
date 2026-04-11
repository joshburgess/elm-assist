use std::collections::HashSet;

use elm_ast::declaration::Declaration;
use elm_ast::expr::{Expr, LetDeclaration};
use elm_ast::node::Spanned;
use elm_ast::pattern::Pattern;

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports let-bound variables that are never used.
pub struct NoUnusedVariables;

impl Rule for NoUnusedVariables {
    fn name(&self) -> &'static str {
        "NoUnusedVariables"
    }

    fn description(&self) -> &'static str {
        "Let bindings that are never used"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut errors = Vec::new();

        for decl in &ctx.module.declarations {
            if let Declaration::FunctionDeclaration(func) = &decl.value {
                check_expr(&func.declaration.value.body, &mut errors);
            }
        }

        errors
    }
}

fn check_expr(expr: &Spanned<Expr>, errors: &mut Vec<LintError>) {
    if let Expr::LetIn { declarations, body } = &expr.value {
        // Collect all names defined in this let block.
        let mut defined: Vec<(String, elm_ast::span::Span)> = Vec::new();
        for decl in declarations {
            match &decl.value {
                LetDeclaration::Function(func) => {
                    defined.push((
                        func.declaration.value.name.value.clone(),
                        func.declaration.value.name.span,
                    ));
                }
                LetDeclaration::Destructuring { pattern, .. } => {
                    collect_pattern_names(&pattern.value, &mut defined);
                }
            }
        }

        // Collect all references in the body and in other let declarations.
        let mut refs = HashSet::new();
        collect_refs_expr(&body.value, &mut refs);
        for decl in declarations {
            match &decl.value {
                LetDeclaration::Function(func) => {
                    collect_refs_expr(&func.declaration.value.body.value, &mut refs);
                }
                LetDeclaration::Destructuring { body: b, .. } => {
                    collect_refs_expr(&b.value, &mut refs);
                }
            }
        }

        for (name, span) in &defined {
            if !refs.contains(name) && !name.starts_with('_') {
                errors.push(LintError {
                    rule: "NoUnusedVariables",
                    severity: Severity::Warning,
                    message: format!("Let binding `{name}` is never used"),
                    span: *span,
                    fix: Some(Fix::replace(*span, format!("_{name}"))),
                });
            }
        }

        // Recurse into sub-expressions.
        for decl in declarations {
            match &decl.value {
                LetDeclaration::Function(func) => {
                    check_expr(&func.declaration.value.body, errors);
                }
                LetDeclaration::Destructuring { body: b, .. } => {
                    check_expr(b, errors);
                }
            }
        }
        check_expr(body, errors);
    }

    // Also recurse into non-let expressions to find nested lets.
    match &expr.value {
        Expr::IfElse {
            branches,
            else_branch,
        } => {
            for (c, b) in branches {
                check_expr(c, errors);
                check_expr(b, errors);
            }
            check_expr(else_branch, errors);
        }
        Expr::CaseOf {
            expr: subject,
            branches,
        } => {
            check_expr(subject, errors);
            for b in branches {
                check_expr(&b.body, errors);
            }
        }
        Expr::Lambda { body, .. } => check_expr(body, errors),
        Expr::Application(args) => {
            for a in args {
                check_expr(a, errors);
            }
        }
        Expr::OperatorApplication { left, right, .. } => {
            check_expr(left, errors);
            check_expr(right, errors);
        }
        Expr::Parenthesized(inner) => check_expr(inner, errors),
        Expr::Negation(inner) => check_expr(inner, errors),
        Expr::Tuple(elems) | Expr::List(elems) => {
            for e in elems {
                check_expr(e, errors);
            }
        }
        _ => {}
    }
}

fn collect_pattern_names(_pat: &Pattern, _out: &mut Vec<(String, elm_ast::span::Span)>) {
    // Destructuring patterns in let blocks — handled minimally for now.
    // A full implementation would extract all variable names from the pattern.
}

fn collect_refs_expr(expr: &Expr, refs: &mut HashSet<String>) {
    match expr {
        Expr::FunctionOrValue { module_name, name } if module_name.is_empty() => {
            refs.insert(name.clone());
        }
        Expr::Application(args) => {
            for a in args {
                collect_refs_expr(&a.value, refs);
            }
        }
        Expr::OperatorApplication { left, right, .. } => {
            collect_refs_expr(&left.value, refs);
            collect_refs_expr(&right.value, refs);
        }
        Expr::IfElse {
            branches,
            else_branch,
        } => {
            for (c, b) in branches {
                collect_refs_expr(&c.value, refs);
                collect_refs_expr(&b.value, refs);
            }
            collect_refs_expr(&else_branch.value, refs);
        }
        Expr::CaseOf {
            expr: subject,
            branches,
        } => {
            collect_refs_expr(&subject.value, refs);
            for b in branches {
                collect_refs_expr(&b.body.value, refs);
            }
        }
        Expr::LetIn { declarations, body } => {
            for d in declarations {
                match &d.value {
                    LetDeclaration::Function(f) => {
                        collect_refs_expr(&f.declaration.value.body.value, refs);
                    }
                    LetDeclaration::Destructuring { body: b, .. } => {
                        collect_refs_expr(&b.value, refs);
                    }
                }
            }
            collect_refs_expr(&body.value, refs);
        }
        Expr::Lambda { body, .. } => collect_refs_expr(&body.value, refs),
        Expr::Parenthesized(inner) | Expr::Negation(inner) => {
            collect_refs_expr(&inner.value, refs);
        }
        Expr::Tuple(elems) | Expr::List(elems) => {
            for e in elems {
                collect_refs_expr(&e.value, refs);
            }
        }
        Expr::Record(fields) => {
            for f in fields {
                collect_refs_expr(&f.value.value.value, refs);
            }
        }
        Expr::RecordUpdate { updates, .. } => {
            for f in updates {
                collect_refs_expr(&f.value.value.value, refs);
            }
        }
        Expr::RecordAccess { record, .. } => {
            collect_refs_expr(&record.value, refs);
        }
        _ => {}
    }
}
