use std::collections::HashSet;

use elm_ast::declaration::Declaration;
use elm_ast::expr::{Expr, LetDeclaration};
use elm_ast::node::Spanned;
use elm_ast::pattern::Pattern;

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports names ending with `_` when the non-underscored version is not in scope.
pub struct NoUnnecessaryTrailingUnderscore;

impl Rule for NoUnnecessaryTrailingUnderscore {
    fn name(&self) -> &'static str {
        "NoUnnecessaryTrailingUnderscore"
    }

    fn description(&self) -> &'static str {
        "Trailing underscores should only be used to avoid shadowing"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut errors = Vec::new();

        // Collect top-level names.
        let mut top_level: HashSet<String> = HashSet::new();
        for decl in &ctx.module.declarations {
            match &decl.value {
                Declaration::FunctionDeclaration(func) => {
                    top_level.insert(func.declaration.value.name.value.clone());
                }
                Declaration::AliasDeclaration(alias) => {
                    top_level.insert(alias.name.value.clone());
                }
                Declaration::CustomTypeDeclaration(ct) => {
                    top_level.insert(ct.name.value.clone());
                }
                Declaration::PortDeclaration(sig) => {
                    top_level.insert(sig.name.value.clone());
                }
                _ => {}
            }
        }

        for decl in &ctx.module.declarations {
            if let Declaration::FunctionDeclaration(func) = &decl.value {
                let mut scope = top_level.clone();

                // Check function args.
                for arg in &func.declaration.value.args {
                    check_pattern_trailing(arg, &scope, &mut errors);
                    collect_names(&arg.value, &mut scope);
                }

                // Check body.
                check_expr_trailing(&func.declaration.value.body, &scope, &mut errors);
            }
        }

        errors
    }
}

fn check_pattern_trailing(
    pattern: &Spanned<Pattern>,
    scope: &HashSet<String>,
    errors: &mut Vec<LintError>,
) {
    match &pattern.value {
        Pattern::Var(name) => {
            check_trailing_name(name, pattern.span, scope, errors);
        }
        Pattern::Tuple(pats) | Pattern::List(pats) => {
            for p in pats {
                check_pattern_trailing(p, scope, errors);
            }
        }
        Pattern::Constructor { args, .. } => {
            for p in args {
                check_pattern_trailing(p, scope, errors);
            }
        }
        Pattern::Record(fields) => {
            for f in fields {
                check_trailing_name(&f.value, f.span, scope, errors);
            }
        }
        Pattern::Cons { head, tail } => {
            check_pattern_trailing(head, scope, errors);
            check_pattern_trailing(tail, scope, errors);
        }
        Pattern::As {
            pattern: inner,
            name,
        } => {
            check_pattern_trailing(inner, scope, errors);
            check_trailing_name(&name.value, name.span, scope, errors);
        }
        Pattern::Parenthesized(inner) => {
            check_pattern_trailing(inner, scope, errors);
        }
        _ => {}
    }
}

fn check_trailing_name(
    name: &str,
    span: elm_ast::span::Span,
    scope: &HashSet<String>,
    errors: &mut Vec<LintError>,
) {
    if let Some(stripped) = name.strip_suffix('_') {
        if !stripped.is_empty() && !scope.contains(stripped) {
            errors.push(LintError {
                rule: "NoUnnecessaryTrailingUnderscore",
                severity: Severity::Warning,
                message: format!(
                    "`{name}` has an unnecessary trailing underscore — `{stripped}` is not in scope"
                ),
                span,
                // No auto-fix: renaming requires updating all references in the body.
                fix: None,
            });
        }
    }
}

fn collect_names(pat: &Pattern, scope: &mut HashSet<String>) {
    match pat {
        Pattern::Var(name) => {
            scope.insert(name.clone());
        }
        Pattern::Tuple(pats) | Pattern::List(pats) => {
            for p in pats {
                collect_names(&p.value, scope);
            }
        }
        Pattern::Constructor { args, .. } => {
            for p in args {
                collect_names(&p.value, scope);
            }
        }
        Pattern::Record(fields) => {
            for f in fields {
                scope.insert(f.value.clone());
            }
        }
        Pattern::Cons { head, tail } => {
            collect_names(&head.value, scope);
            collect_names(&tail.value, scope);
        }
        Pattern::As {
            pattern: inner,
            name,
        } => {
            collect_names(&inner.value, scope);
            scope.insert(name.value.clone());
        }
        Pattern::Parenthesized(inner) => {
            collect_names(&inner.value, scope);
        }
        _ => {}
    }
}

fn check_expr_trailing(expr: &Spanned<Expr>, scope: &HashSet<String>, errors: &mut Vec<LintError>) {
    match &expr.value {
        Expr::LetIn { declarations, body } => {
            let mut inner_scope = scope.clone();
            for decl in declarations {
                match &decl.value {
                    LetDeclaration::Function(func) => {
                        let name = &func.declaration.value.name;
                        check_trailing_name(&name.value, name.span, &inner_scope, errors);
                        inner_scope.insert(name.value.clone());

                        let mut fn_scope = inner_scope.clone();
                        for arg in &func.declaration.value.args {
                            check_pattern_trailing(arg, &fn_scope, errors);
                            collect_names(&arg.value, &mut fn_scope);
                        }
                        check_expr_trailing(&func.declaration.value.body, &fn_scope, errors);
                    }
                    LetDeclaration::Destructuring { pattern, body: b } => {
                        check_pattern_trailing(pattern, &inner_scope, errors);
                        collect_names(&pattern.value, &mut inner_scope);
                        check_expr_trailing(b, &inner_scope, errors);
                    }
                }
            }
            check_expr_trailing(body, &inner_scope, errors);
        }
        Expr::Lambda { args, body } => {
            let mut inner_scope = scope.clone();
            for arg in args {
                check_pattern_trailing(arg, &inner_scope, errors);
                collect_names(&arg.value, &mut inner_scope);
            }
            check_expr_trailing(body, &inner_scope, errors);
        }
        Expr::CaseOf {
            expr: subject,
            branches,
        } => {
            check_expr_trailing(subject, scope, errors);
            for branch in branches {
                let mut branch_scope = scope.clone();
                check_pattern_trailing(&branch.pattern, &branch_scope, errors);
                collect_names(&branch.pattern.value, &mut branch_scope);
                check_expr_trailing(&branch.body, &branch_scope, errors);
            }
        }
        Expr::IfElse {
            branches,
            else_branch,
        } => {
            for (cond, body) in branches {
                check_expr_trailing(cond, scope, errors);
                check_expr_trailing(body, scope, errors);
            }
            check_expr_trailing(else_branch, scope, errors);
        }
        Expr::Application(args) => {
            for a in args {
                check_expr_trailing(a, scope, errors);
            }
        }
        Expr::OperatorApplication { left, right, .. } => {
            check_expr_trailing(left, scope, errors);
            check_expr_trailing(right, scope, errors);
        }
        Expr::Parenthesized(inner) | Expr::Negation(inner) => {
            check_expr_trailing(inner, scope, errors);
        }
        Expr::Tuple(elems) | Expr::List(elems) => {
            for e in elems {
                check_expr_trailing(e, scope, errors);
            }
        }
        Expr::Record(fields) => {
            for f in fields {
                check_expr_trailing(&f.value.value, scope, errors);
            }
        }
        Expr::RecordUpdate { updates, .. } => {
            for f in updates {
                check_expr_trailing(&f.value.value, scope, errors);
            }
        }
        Expr::RecordAccess { record, .. } => {
            check_expr_trailing(record, scope, errors);
        }
        _ => {}
    }
}
