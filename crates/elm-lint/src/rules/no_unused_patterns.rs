use std::collections::HashSet;

use elm_ast::expr::{CaseBranch, Expr, LetDeclaration};
use elm_ast::node::Spanned;
use elm_ast::pattern::Pattern;
use elm_ast::visit::{self, Visit};

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports pattern variables that are extracted but never used in the branch body.
pub struct NoUnusedPatterns;

impl Rule for NoUnusedPatterns {
    fn name(&self) -> &'static str {
        "NoUnusedPatterns"
    }

    fn description(&self) -> &'static str {
        "Pattern variables should be used or replaced with _"
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
    fn visit_case_branch(&mut self, branch: &CaseBranch) {
        let mut names = Vec::new();
        collect_pattern_bindings(&branch.pattern, &mut names);

        if !names.is_empty() {
            let refs = collect_refs_from_expr(&branch.body);
            for (name, span) in &names {
                if !refs.contains(name) {
                    self.errors.push(LintError {
                        rule: "NoUnusedPatterns",
                        severity: Severity::Warning,
                        message: format!("Pattern variable `{name}` is never used in this branch"),
                        span: *span,
                        fix: None,
                    });
                }
            }
        }

        visit::walk_case_branch(self, branch);
    }

    fn visit_let_declaration(&mut self, decl: &Spanned<LetDeclaration>) {
        if let LetDeclaration::Destructuring { pattern, body } = &decl.value {
            let mut names = Vec::new();
            collect_pattern_bindings(pattern, &mut names);

            // For let destructuring, we can't easily check usage since the names
            // are used in the let body (not this expression). Skip for now —
            // NoUnusedVariables handles this.
            let _ = (names, body);
        }
        visit::walk_let_declaration(self, decl);
    }
}

fn collect_pattern_bindings(
    pattern: &Spanned<Pattern>,
    out: &mut Vec<(String, elm_ast::span::Span)>,
) {
    match &pattern.value {
        Pattern::Var(name) => {
            out.push((name.clone(), pattern.span));
        }
        Pattern::Tuple(pats) | Pattern::List(pats) => {
            for p in pats {
                collect_pattern_bindings(p, out);
            }
        }
        Pattern::Constructor { args, .. } => {
            for p in args {
                collect_pattern_bindings(p, out);
            }
        }
        Pattern::Record(fields) => {
            for f in fields {
                out.push((f.value.clone(), f.span));
            }
        }
        Pattern::Cons { head, tail } => {
            collect_pattern_bindings(head, out);
            collect_pattern_bindings(tail, out);
        }
        Pattern::As {
            pattern: inner,
            name,
        } => {
            collect_pattern_bindings(inner, out);
            out.push((name.value.clone(), name.span));
        }
        Pattern::Parenthesized(inner) => {
            collect_pattern_bindings(inner, out);
        }
        Pattern::Anything | Pattern::Literal(_) | Pattern::Unit | Pattern::Hex(_) => {}
    }
}

fn collect_refs_from_expr(expr: &Spanned<Expr>) -> HashSet<String> {
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
