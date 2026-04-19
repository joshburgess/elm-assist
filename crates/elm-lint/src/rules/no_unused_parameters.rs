use std::collections::HashSet;

use elm_ast::declaration::Declaration;
use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::pattern::Pattern;

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports function parameters that are always bound with `_`.
pub struct NoUnusedParameters;

impl Rule for NoUnusedParameters {
    fn name(&self) -> &'static str {
        "NoUnusedParameters"
    }

    fn description(&self) -> &'static str {
        "Function parameters that are never used should be removed or the function simplified"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut errors = Vec::new();

        for decl in &ctx.module.declarations {
            if let Declaration::FunctionDeclaration(func) = &decl.value {
                let impl_node = &func.declaration.value;

                // Collect all referenced names in the body.
                let mut refs = HashSet::new();
                collect_refs(&impl_node.body.value, &mut refs);

                // Check each parameter.
                for arg in &impl_node.args {
                    check_unused_param(ctx.source, arg, &refs, &mut errors);
                }
            }
        }

        errors
    }
}

fn check_unused_param(
    source: &str,
    pattern: &Spanned<Pattern>,
    refs: &HashSet<String>,
    errors: &mut Vec<LintError>,
) {
    match &pattern.value {
        Pattern::Var(name) => {
            if !refs.contains(name) {
                // Fix: replace with `_`
                errors.push(LintError {
                    rule: "NoUnusedParameters",
                    severity: Severity::Warning,
                    message: format!("Parameter `{name}` is never used"),
                    span: pattern.span,
                    fix: Some(Fix::replace(pattern.span, "_".to_string())),
                });
            }
        }
        Pattern::As {
            pattern: inner,
            name,
        } => {
            let inner_used = pattern_has_used_names(&inner.value, refs);
            let as_used = refs.contains(&name.value);

            if !inner_used && !as_used {
                errors.push(LintError {
                    rule: "NoUnusedParameters",
                    severity: Severity::Warning,
                    message: format!(
                        "Parameter `{} as {}` is never used",
                        &source[inner.span.start.offset..inner.span.end.offset],
                        name.value
                    ),
                    span: pattern.span,
                    fix: Some(Fix::replace(pattern.span, "_".to_string())),
                });
            }
        }
        Pattern::Tuple(pats) => {
            let any_used = pats.iter().any(|p| pattern_has_used_names(&p.value, refs));
            if !any_used {
                errors.push(LintError {
                    rule: "NoUnusedParameters",
                    severity: Severity::Warning,
                    message: "Tuple parameter is never used".to_string(),
                    span: pattern.span,
                    fix: Some(Fix::replace(pattern.span, "_".to_string())),
                });
            }
        }
        Pattern::Record(fields) => {
            let any_used = fields.iter().any(|f| refs.contains(&f.value));
            if !any_used {
                errors.push(LintError {
                    rule: "NoUnusedParameters",
                    severity: Severity::Warning,
                    message: "Record parameter is never used".to_string(),
                    span: pattern.span,
                    fix: Some(Fix::replace(pattern.span, "_".to_string())),
                });
            }
        }
        Pattern::Constructor { args, .. } => {
            // Don't flag constructor patterns as unused — the pattern match itself is meaningful.
            let _ = args;
        }
        // `_` and `()` are already wildcards/units — not flagged.
        Pattern::Anything | Pattern::Unit => {}
        Pattern::Parenthesized(inner) => {
            check_unused_param(source, inner, refs, errors);
        }
        _ => {}
    }
}

fn pattern_has_used_names(pat: &Pattern, refs: &HashSet<String>) -> bool {
    match pat {
        Pattern::Var(name) => refs.contains(name),
        Pattern::Tuple(pats) | Pattern::List(pats) => {
            pats.iter().any(|p| pattern_has_used_names(&p.value, refs))
        }
        Pattern::Constructor { args, .. } => {
            args.iter().any(|p| pattern_has_used_names(&p.value, refs))
        }
        Pattern::Record(fields) => fields.iter().any(|f| refs.contains(&f.value)),
        Pattern::Cons { head, tail } => {
            pattern_has_used_names(&head.value, refs) || pattern_has_used_names(&tail.value, refs)
        }
        Pattern::As {
            pattern: inner,
            name,
        } => pattern_has_used_names(&inner.value, refs) || refs.contains(&name.value),
        Pattern::Parenthesized(inner) => pattern_has_used_names(&inner.value, refs),
        Pattern::Anything | Pattern::Literal(_) | Pattern::Unit | Pattern::Hex(_) => false,
    }
}

fn collect_refs(expr: &Expr, refs: &mut HashSet<String>) {
    match expr {
        Expr::FunctionOrValue { module_name, name } if module_name.is_empty() => {
            refs.insert(name.clone());
        }
        Expr::Application(args) => {
            for a in args {
                collect_refs(&a.value, refs);
            }
        }
        Expr::OperatorApplication { left, right, .. } => {
            collect_refs(&left.value, refs);
            collect_refs(&right.value, refs);
        }
        Expr::IfElse {
            branches,
            else_branch,
        } => {
            for branch in branches {
                collect_refs(&branch.condition.value, refs);
                collect_refs(&branch.then_branch.value, refs);
            }
            collect_refs(&else_branch.value, refs);
        }
        Expr::CaseOf {
            expr: subject,
            branches,
        } => {
            collect_refs(&subject.value, refs);
            for b in branches {
                collect_pattern_refs(&b.pattern.value, refs);
                collect_refs(&b.body.value, refs);
            }
        }
        Expr::LetIn {
            declarations, body, ..
        } => {
            for d in declarations {
                match &d.value {
                    elm_ast::expr::LetDeclaration::Function(f) => {
                        for arg in &f.declaration.value.args {
                            collect_pattern_refs(&arg.value, refs);
                        }
                        collect_refs(&f.declaration.value.body.value, refs);
                    }
                    elm_ast::expr::LetDeclaration::Destructuring { pattern, body: b } => {
                        collect_pattern_refs(&pattern.value, refs);
                        collect_refs(&b.value, refs);
                    }
                }
            }
            collect_refs(&body.value, refs);
        }
        Expr::Lambda { args, body } => {
            for arg in args {
                collect_pattern_refs(&arg.value, refs);
            }
            collect_refs(&body.value, refs);
        }
        Expr::Parenthesized { expr: inner, .. } | Expr::Negation(inner) => {
            collect_refs(&inner.value, refs);
        }
        Expr::Tuple(elems) => {
            for e in elems {
                collect_refs(&e.value, refs);
            }
        }
        Expr::List {
            elements: elems, ..
        } => {
            for e in elems {
                collect_refs(&e.value, refs);
            }
        }
        Expr::Record(fields) => {
            for f in fields {
                collect_refs(&f.value.value.value, refs);
            }
        }
        Expr::RecordUpdate { updates, .. } => {
            for f in updates {
                collect_refs(&f.value.value.value, refs);
            }
        }
        Expr::RecordAccess { record, .. } => {
            collect_refs(&record.value, refs);
        }
        _ => {}
    }
}

fn collect_pattern_refs(pat: &Pattern, refs: &mut HashSet<String>) {
    match pat {
        Pattern::Var(name) => {
            refs.insert(name.clone());
        }
        Pattern::Tuple(pats) | Pattern::List(pats) => {
            for p in pats {
                collect_pattern_refs(&p.value, refs);
            }
        }
        Pattern::Constructor { args, .. } => {
            for p in args {
                collect_pattern_refs(&p.value, refs);
            }
        }
        Pattern::Record(fields) => {
            for f in fields {
                refs.insert(f.value.clone());
            }
        }
        Pattern::Cons { head, tail } => {
            collect_pattern_refs(&head.value, refs);
            collect_pattern_refs(&tail.value, refs);
        }
        Pattern::As {
            pattern: inner,
            name,
        } => {
            collect_pattern_refs(&inner.value, refs);
            refs.insert(name.value.clone());
        }
        Pattern::Parenthesized(inner) => {
            collect_pattern_refs(&inner.value, refs);
        }
        _ => {}
    }
}
