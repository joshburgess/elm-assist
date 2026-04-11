use std::collections::HashSet;

use elm_ast::expr::{Expr, LetDeclaration};
use elm_ast::node::Spanned;
use elm_ast::pattern::Pattern;
use elm_ast::visit::{self, Visit};

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports let bindings whose value is only used in one branch of an if/case.
/// The binding should be moved into that branch to avoid unnecessary computation.
pub struct NoPrematureLetComputation;

impl Rule for NoPrematureLetComputation {
    fn name(&self) -> &'static str {
        "NoPrematureLetComputation"
    }

    fn description(&self) -> &'static str {
        "Let bindings used in only one branch of if/case should be moved into that branch"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut visitor = Visitor {
            errors: Vec::new(),
        };
        visitor.visit_module(ctx.module);
        visitor.errors
    }
}

struct Visitor {
    errors: Vec<LintError>,
}

impl Visit for Visitor {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::LetIn { declarations, body } = &expr.value {
            // Only check if the body is an if/case.
            match &body.value {
                Expr::IfElse {
                    branches,
                    else_branch,
                } => {
                    let mut branch_exprs: Vec<&Spanned<Expr>> = Vec::new();
                    for (cond, body) in branches {
                        branch_exprs.push(cond);
                        branch_exprs.push(body);
                    }
                    branch_exprs.push(else_branch);
                    check_let_bindings_in_branches(declarations, &branch_exprs, &mut self.errors);
                }
                Expr::CaseOf {
                    expr: _subject,
                    branches,
                } => {
                    let branch_exprs: Vec<&Spanned<Expr>> =
                        branches.iter().map(|b| &b.body).collect();
                    check_let_bindings_in_branches(declarations, &branch_exprs, &mut self.errors);
                }
                _ => {}
            }
        }
        visit::walk_expr(self, expr);
    }
}

fn check_let_bindings_in_branches(
    declarations: &[Spanned<LetDeclaration>],
    branch_exprs: &[&Spanned<Expr>],
    errors: &mut Vec<LintError>,
) {
    // For each let binding, check if it's used in only one branch.
    for decl in declarations {
        let (names, decl_span) = match &decl.value {
            LetDeclaration::Function(func) => {
                // Only check simple bindings (no args) — functions with args are cheap.
                if !func.declaration.value.args.is_empty() {
                    continue;
                }
                let name = func.declaration.value.name.value.clone();
                (vec![name], func.declaration.value.name.span)
            }
            LetDeclaration::Destructuring { pattern, .. } => {
                let mut names = Vec::new();
                collect_pattern_names(&pattern.value, &mut names);
                if names.is_empty() {
                    continue;
                }
                (names, pattern.span)
            }
        };

        // Also check if the binding references other let bindings — skip if it does,
        // since moving it might break other references.
        let other_let_names: HashSet<String> = declarations
            .iter()
            .filter_map(|d| match &d.value {
                LetDeclaration::Function(f) => {
                    Some(f.declaration.value.name.value.clone())
                }
                _ => None,
            })
            .collect();

        let binding_body = match &decl.value {
            LetDeclaration::Function(f) => &f.declaration.value.body,
            LetDeclaration::Destructuring { body, .. } => body,
        };

        let binding_refs = collect_refs(binding_body);
        if binding_refs.iter().any(|r| other_let_names.contains(r) && !names.contains(r)) {
            continue;
        }

        // Count how many branches use any of the binding's names.
        let mut usage_count = 0;
        for branch_expr in branch_exprs {
            let branch_refs = collect_refs(branch_expr);
            if names.iter().any(|n| branch_refs.contains(n)) {
                usage_count += 1;
            }
        }

        if usage_count == 1 && branch_exprs.len() > 1 {
            let name_display = names.join(", ");
            errors.push(LintError {
                rule: "NoPrematureLetComputation",
                severity: Severity::Warning,
                message: format!(
                    "`{name_display}` is only used in one branch — move it there to avoid unnecessary computation"
                ),
                span: decl_span,
                fix: None,
            });
        }
    }
}

fn collect_pattern_names(pat: &Pattern, names: &mut Vec<String>) {
    match pat {
        Pattern::Var(name) => names.push(name.clone()),
        Pattern::Tuple(pats) | Pattern::List(pats) => {
            for p in pats {
                collect_pattern_names(&p.value, names);
            }
        }
        Pattern::Constructor { args, .. } => {
            for p in args {
                collect_pattern_names(&p.value, names);
            }
        }
        Pattern::Record(fields) => {
            for f in fields {
                names.push(f.value.clone());
            }
        }
        Pattern::Cons { head, tail } => {
            collect_pattern_names(&head.value, names);
            collect_pattern_names(&tail.value, names);
        }
        Pattern::As { pattern: inner, name } => {
            collect_pattern_names(&inner.value, names);
            names.push(name.value.clone());
        }
        Pattern::Parenthesized(inner) => {
            collect_pattern_names(&inner.value, names);
        }
        _ => {}
    }
}

fn collect_refs(expr: &Spanned<Expr>) -> HashSet<String> {
    let mut refs = HashSet::new();
    collect_refs_inner(&expr.value, &mut refs);
    refs
}

fn collect_refs_inner(expr: &Expr, refs: &mut HashSet<String>) {
    match expr {
        Expr::FunctionOrValue { module_name, name } if module_name.is_empty() => {
            refs.insert(name.clone());
        }
        Expr::Application(args) => {
            for a in args {
                collect_refs_inner(&a.value, refs);
            }
        }
        Expr::OperatorApplication { left, right, .. } => {
            collect_refs_inner(&left.value, refs);
            collect_refs_inner(&right.value, refs);
        }
        Expr::IfElse {
            branches,
            else_branch,
        } => {
            for (c, b) in branches {
                collect_refs_inner(&c.value, refs);
                collect_refs_inner(&b.value, refs);
            }
            collect_refs_inner(&else_branch.value, refs);
        }
        Expr::CaseOf {
            expr: subject,
            branches,
        } => {
            collect_refs_inner(&subject.value, refs);
            for b in branches {
                collect_refs_inner(&b.body.value, refs);
            }
        }
        Expr::LetIn { declarations, body } => {
            for d in declarations {
                match &d.value {
                    LetDeclaration::Function(f) => {
                        collect_refs_inner(&f.declaration.value.body.value, refs);
                    }
                    LetDeclaration::Destructuring { body: b, .. } => {
                        collect_refs_inner(&b.value, refs);
                    }
                }
            }
            collect_refs_inner(&body.value, refs);
        }
        Expr::Lambda { body, .. } => collect_refs_inner(&body.value, refs),
        Expr::Parenthesized(inner) | Expr::Negation(inner) => {
            collect_refs_inner(&inner.value, refs);
        }
        Expr::Tuple(elems) | Expr::List(elems) => {
            for e in elems {
                collect_refs_inner(&e.value, refs);
            }
        }
        Expr::Record(fields) => {
            for f in fields {
                collect_refs_inner(&f.value.value.value, refs);
            }
        }
        Expr::RecordUpdate { updates, .. } => {
            for f in updates {
                collect_refs_inner(&f.value.value.value, refs);
            }
        }
        Expr::RecordAccess { record, .. } => {
            collect_refs_inner(&record.value, refs);
        }
        _ => {}
    }
}
