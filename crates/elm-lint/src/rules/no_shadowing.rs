use std::collections::HashSet;

use elm_ast::declaration::Declaration;
use elm_ast::expr::{Expr, LetDeclaration};
use elm_ast::node::Spanned;
use elm_ast::pattern::Pattern;

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports local bindings that shadow an outer name.
pub struct NoShadowing;

impl Rule for NoShadowing {
    fn name(&self) -> &'static str {
        "NoShadowing"
    }

    fn description(&self) -> &'static str {
        "Local bindings should not shadow names from an outer scope"
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
                    for ctor in &ct.constructors {
                        top_level.insert(ctor.value.name.value.clone());
                    }
                }
                Declaration::PortDeclaration(sig) => {
                    top_level.insert(sig.name.value.clone());
                }
                _ => {}
            }
        }

        // For each function, check parameters and let bindings for shadowing.
        for decl in &ctx.module.declarations {
            if let Declaration::FunctionDeclaration(func) = &decl.value {
                let func_name = &func.declaration.value.name.value;
                let mut scope: HashSet<String> = top_level.clone();
                scope.remove(func_name); // Function can't shadow itself.

                // Check function parameters.
                for arg in &func.declaration.value.args {
                    check_pattern_shadowing(arg, &scope, &mut errors);
                    collect_pattern_names(&arg.value, &mut scope);
                }

                // Check body expressions.
                check_expr_shadowing(&func.declaration.value.body, &scope, &mut errors);
            }
        }

        errors
    }
}

fn check_pattern_shadowing(
    pattern: &Spanned<Pattern>,
    scope: &HashSet<String>,
    errors: &mut Vec<LintError>,
) {
    match &pattern.value {
        Pattern::Var(name) => {
            if scope.contains(name) {
                errors.push(LintError {
                    rule: "NoShadowing",
                    severity: Severity::Warning,
                    message: format!("`{name}` shadows a name from an outer scope"),
                    span: pattern.span,
                    fix: None,
                });
            }
        }
        Pattern::Tuple(pats) | Pattern::List(pats) => {
            for p in pats {
                check_pattern_shadowing(p, scope, errors);
            }
        }
        Pattern::Constructor { args, .. } => {
            for p in args {
                check_pattern_shadowing(p, scope, errors);
            }
        }
        Pattern::Record(fields) => {
            for f in fields {
                if scope.contains(&f.value) {
                    errors.push(LintError {
                        rule: "NoShadowing",
                        severity: Severity::Warning,
                        message: format!("`{}` shadows a name from an outer scope", f.value),
                        span: f.span,
                        fix: None,
                    });
                }
            }
        }
        Pattern::Cons { head, tail } => {
            check_pattern_shadowing(head, scope, errors);
            check_pattern_shadowing(tail, scope, errors);
        }
        Pattern::As {
            pattern: inner,
            name,
        } => {
            check_pattern_shadowing(inner, scope, errors);
            if scope.contains(&name.value) {
                errors.push(LintError {
                    rule: "NoShadowing",
                    severity: Severity::Warning,
                    message: format!("`{}` shadows a name from an outer scope", name.value),
                    span: name.span,
                    fix: None,
                });
            }
        }
        Pattern::Parenthesized(inner) => {
            check_pattern_shadowing(inner, scope, errors);
        }
        Pattern::Anything | Pattern::Literal(_) | Pattern::Unit | Pattern::Hex(_) => {}
    }
}

fn collect_pattern_names(pat: &Pattern, scope: &mut HashSet<String>) {
    match pat {
        Pattern::Var(name) => {
            scope.insert(name.clone());
        }
        Pattern::Tuple(pats) | Pattern::List(pats) => {
            for p in pats {
                collect_pattern_names(&p.value, scope);
            }
        }
        Pattern::Constructor { args, .. } => {
            for p in args {
                collect_pattern_names(&p.value, scope);
            }
        }
        Pattern::Record(fields) => {
            for f in fields {
                scope.insert(f.value.clone());
            }
        }
        Pattern::Cons { head, tail } => {
            collect_pattern_names(&head.value, scope);
            collect_pattern_names(&tail.value, scope);
        }
        Pattern::As {
            pattern: inner,
            name,
        } => {
            collect_pattern_names(&inner.value, scope);
            scope.insert(name.value.clone());
        }
        Pattern::Parenthesized(inner) => {
            collect_pattern_names(&inner.value, scope);
        }
        Pattern::Anything | Pattern::Literal(_) | Pattern::Unit | Pattern::Hex(_) => {}
    }
}

fn check_expr_shadowing(
    expr: &Spanned<Expr>,
    scope: &HashSet<String>,
    errors: &mut Vec<LintError>,
) {
    match &expr.value {
        Expr::LetIn { declarations, body } => {
            let mut inner_scope = scope.clone();
            for decl in declarations {
                match &decl.value {
                    LetDeclaration::Function(func) => {
                        let name = &func.declaration.value.name.value;
                        if inner_scope.contains(name) {
                            errors.push(LintError {
                                rule: "NoShadowing",
                                severity: Severity::Warning,
                                message: format!("`{name}` shadows a name from an outer scope"),
                                span: func.declaration.value.name.span,
                                fix: None,
                            });
                        }
                        inner_scope.insert(name.clone());

                        // Check function args within the let binding.
                        let mut fn_scope = inner_scope.clone();
                        for arg in &func.declaration.value.args {
                            check_pattern_shadowing(arg, &fn_scope, errors);
                            collect_pattern_names(&arg.value, &mut fn_scope);
                        }
                        check_expr_shadowing(&func.declaration.value.body, &fn_scope, errors);
                    }
                    LetDeclaration::Destructuring { pattern, body: b } => {
                        check_pattern_shadowing(pattern, &inner_scope, errors);
                        collect_pattern_names(&pattern.value, &mut inner_scope);
                        check_expr_shadowing(b, &inner_scope, errors);
                    }
                }
            }
            check_expr_shadowing(body, &inner_scope, errors);
        }
        Expr::Lambda { args, body } => {
            let mut inner_scope = scope.clone();
            for arg in args {
                check_pattern_shadowing(arg, &inner_scope, errors);
                collect_pattern_names(&arg.value, &mut inner_scope);
            }
            check_expr_shadowing(body, &inner_scope, errors);
        }
        Expr::CaseOf {
            expr: subject,
            branches,
        } => {
            check_expr_shadowing(subject, scope, errors);
            for branch in branches {
                let mut branch_scope = scope.clone();
                check_pattern_shadowing(&branch.pattern, &branch_scope, errors);
                collect_pattern_names(&branch.pattern.value, &mut branch_scope);
                check_expr_shadowing(&branch.body, &branch_scope, errors);
            }
        }
        Expr::IfElse {
            branches,
            else_branch,
        } => {
            for (cond, body) in branches {
                check_expr_shadowing(cond, scope, errors);
                check_expr_shadowing(body, scope, errors);
            }
            check_expr_shadowing(else_branch, scope, errors);
        }
        Expr::Application(args) => {
            for a in args {
                check_expr_shadowing(a, scope, errors);
            }
        }
        Expr::OperatorApplication { left, right, .. } => {
            check_expr_shadowing(left, scope, errors);
            check_expr_shadowing(right, scope, errors);
        }
        Expr::Parenthesized(inner) | Expr::Negation(inner) => {
            check_expr_shadowing(inner, scope, errors);
        }
        Expr::Tuple(elems) | Expr::List(elems) => {
            for e in elems {
                check_expr_shadowing(e, scope, errors);
            }
        }
        Expr::Record(fields) => {
            for f in fields {
                check_expr_shadowing(&f.value.value, scope, errors);
            }
        }
        Expr::RecordUpdate { updates, .. } => {
            for f in updates {
                check_expr_shadowing(&f.value.value, scope, errors);
            }
        }
        Expr::RecordAccess { record, .. } => {
            check_expr_shadowing(record, scope, errors);
        }
        _ => {}
    }
}
