use elm_ast::declaration::Declaration;
use elm_ast::expr::{Expr, LetDeclaration};
use elm_ast::node::Spanned;

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports recursive functions that are not tail-call optimized.
///
/// In Elm, only tail-recursive functions are optimized by the compiler.
/// Non-TCO recursion can cause stack overflows on large inputs.
///
/// A recursive call is in tail position if it is the last operation —
/// i.e., nothing wraps or transforms its result.
pub struct NoUnoptimizedRecursion;

impl Rule for NoUnoptimizedRecursion {
    fn name(&self) -> &'static str {
        "NoUnoptimizedRecursion"
    }

    fn description(&self) -> &'static str {
        "Recursive functions should use tail-call optimization"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut errors = Vec::new();

        for decl in &ctx.module.declarations {
            if let Declaration::FunctionDeclaration(func) = &decl.value {
                let name = &func.declaration.value.name.value;

                // Check if the function is recursive at all.
                if !contains_self_call(&func.declaration.value.body, name) {
                    continue;
                }

                // Check if all recursive calls are in tail position.
                if !all_calls_in_tail_position(&func.declaration.value.body, name) {
                    errors.push(LintError {
                        rule: self.name(),
                        severity: Severity::Warning,
                        message: format!(
                            "`{name}` is recursive but not all calls are in tail position"
                        ),
                        span: func.declaration.value.name.span,
                        fix: None,
                    });
                }
            }
        }

        errors
    }
}

/// Check if an expression contains any call to `name`.
fn contains_self_call(expr: &Spanned<Expr>, name: &str) -> bool {
    match &expr.value {
        Expr::FunctionOrValue { module_name, name: n } if module_name.is_empty() && n == name => {
            true
        }
        Expr::Application(args) => args.iter().any(|a| contains_self_call(a, name)),
        Expr::OperatorApplication { left, right, .. } => {
            contains_self_call(left, name) || contains_self_call(right, name)
        }
        Expr::IfElse {
            branches,
            else_branch,
        } => {
            branches
                .iter()
                .any(|(c, b)| contains_self_call(c, name) || contains_self_call(b, name))
                || contains_self_call(else_branch, name)
        }
        Expr::CaseOf {
            expr: subject,
            branches,
        } => {
            contains_self_call(subject, name)
                || branches.iter().any(|b| contains_self_call(&b.body, name))
        }
        Expr::LetIn { declarations, body } => {
            declarations.iter().any(|d| match &d.value {
                LetDeclaration::Function(f) => {
                    contains_self_call(&f.declaration.value.body, name)
                }
                LetDeclaration::Destructuring { body: b, .. } => contains_self_call(b, name),
            }) || contains_self_call(body, name)
        }
        Expr::Lambda { body, .. } => contains_self_call(body, name),
        Expr::Parenthesized(inner) | Expr::Negation(inner) => contains_self_call(inner, name),
        Expr::Tuple(elems) | Expr::List(elems) => {
            elems.iter().any(|e| contains_self_call(e, name))
        }
        Expr::Record(fields) => fields
            .iter()
            .any(|f| contains_self_call(&f.value.value, name)),
        Expr::RecordUpdate { updates, .. } => updates
            .iter()
            .any(|f| contains_self_call(&f.value.value, name)),
        Expr::RecordAccess { record, .. } => contains_self_call(record, name),
        _ => false,
    }
}

/// Check that all recursive calls to `name` are in tail position.
/// Returns true if every self-call is in tail position (or there are no self-calls).
fn all_calls_in_tail_position(expr: &Spanned<Expr>, name: &str) -> bool {
    match &expr.value {
        // Application where the function is a self-call: this IS a tail call.
        Expr::Application(args) if is_direct_self_call(args, name) => true,

        // Application where it's NOT a self-call but args might contain one:
        // any self-call inside args is NOT in tail position.
        Expr::Application(args) => !args.iter().any(|a| contains_self_call(a, name)),

        // Operator application: self-calls in operator args are never in tail position.
        Expr::OperatorApplication { left, right, .. } => {
            !contains_self_call(left, name) && !contains_self_call(right, name)
        }

        // if/case: recursive calls in branch bodies CAN be in tail position.
        Expr::IfElse {
            branches,
            else_branch,
        } => {
            // Conditions must not contain recursive calls (not tail position).
            let conds_ok = branches
                .iter()
                .all(|(c, _)| !contains_self_call(c, name));
            // Branch bodies must have all recursive calls in tail position.
            let bodies_ok = branches
                .iter()
                .all(|(_, b)| all_calls_in_tail_position(b, name));
            let else_ok = all_calls_in_tail_position(else_branch, name);
            conds_ok && bodies_ok && else_ok
        }
        Expr::CaseOf {
            expr: subject,
            branches,
        } => {
            let subject_ok = !contains_self_call(subject, name);
            let branches_ok = branches
                .iter()
                .all(|b| all_calls_in_tail_position(&b.body, name));
            subject_ok && branches_ok
        }

        // let..in: only the body is in tail position.
        Expr::LetIn { declarations, body } => {
            let decls_ok = declarations.iter().all(|d| match &d.value {
                LetDeclaration::Function(f) => {
                    !contains_self_call(&f.declaration.value.body, name)
                }
                LetDeclaration::Destructuring { body: b, .. } => !contains_self_call(b, name),
            });
            decls_ok && all_calls_in_tail_position(body, name)
        }

        // Lambda body: self-calls inside are not in the outer function's tail position.
        Expr::Lambda { body, .. } => !contains_self_call(body, name),

        // Parenthesized: transparent
        Expr::Parenthesized(inner) => all_calls_in_tail_position(inner, name),

        // Everything else: no self-calls possible or not in tail position.
        Expr::Negation(inner) => !contains_self_call(inner, name),
        Expr::Tuple(elems) | Expr::List(elems) => {
            !elems.iter().any(|e| contains_self_call(e, name))
        }
        Expr::Record(fields) => !fields
            .iter()
            .any(|f| contains_self_call(&f.value.value, name)),
        Expr::RecordUpdate { updates, .. } => !updates
            .iter()
            .any(|f| contains_self_call(&f.value.value, name)),
        Expr::RecordAccess { record, .. } => !contains_self_call(record, name),
        _ => true,
    }
}

fn is_direct_self_call(args: &[Spanned<Expr>], name: &str) -> bool {
    if let Some(first) = args.first() {
        matches!(
            &first.value,
            Expr::FunctionOrValue { module_name, name: n } if module_name.is_empty() && n == name
        )
    } else {
        false
    }
}
