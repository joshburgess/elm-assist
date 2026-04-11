use elm_ast::declaration::Declaration;
use elm_ast::expr::{Expr, LetDeclaration};
use elm_ast::node::Spanned;
use elm_ast::pattern::Pattern;

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports functions that exceed a configurable cognitive complexity threshold.
///
/// Cognitive complexity measures how hard a function is to understand by counting:
/// - +1 for each branching statement (if, case, &&, ||)
/// - +1 for each level of nesting when branching
/// - +1 for recursion
pub struct CognitiveComplexity {
    pub threshold: u32,
}

impl Default for CognitiveComplexity {
    fn default() -> Self {
        Self { threshold: 15 }
    }
}

impl Rule for CognitiveComplexity {
    fn name(&self) -> &'static str {
        "CognitiveComplexity"
    }

    fn description(&self) -> &'static str {
        "Functions should not exceed the cognitive complexity threshold"
    }

    fn configure(&mut self, options: &toml::Value) -> Result<(), String> {
        if let Some(val) = options.get("threshold") {
            self.threshold = val
                .as_integer()
                .ok_or_else(|| "threshold must be an integer".to_string())?
                as u32;
        }
        Ok(())
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut errors = Vec::new();

        for decl in &ctx.module.declarations {
            if let Declaration::FunctionDeclaration(func) = &decl.value {
                let name = &func.declaration.value.name.value;
                let complexity = compute_complexity(&func.declaration.value.body, name, 0);

                if complexity > self.threshold {
                    errors.push(LintError {
                        rule: self.name(),
                        severity: Severity::Warning,
                        message: format!(
                            "`{name}` has a cognitive complexity of {complexity} (threshold is {})",
                            self.threshold
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

fn compute_complexity(expr: &Spanned<Expr>, func_name: &str, nesting: u32) -> u32 {
    match &expr.value {
        Expr::IfElse {
            branches,
            else_branch,
        } => {
            let mut cost = 0;
            for (i, (cond, body)) in branches.iter().enumerate() {
                if i == 0 {
                    // First `if`: +1 for the branch, +nesting for depth
                    cost += 1 + nesting;
                } else {
                    // `else if`: +1 only (no nesting penalty for else-if chains)
                    cost += 1;
                }
                cost += compute_complexity(cond, func_name, nesting + 1);
                cost += compute_complexity(body, func_name, nesting + 1);
            }
            // `else`: no inherent cost (it's the default path), but count its body
            cost += compute_complexity(else_branch, func_name, nesting + 1);
            cost
        }
        Expr::CaseOf {
            expr: subject,
            branches,
        } => {
            // +1 for the case, +nesting
            let mut cost = 1 + nesting;
            cost += compute_complexity(subject, func_name, nesting);
            for branch in branches {
                cost += compute_complexity(&branch.body, func_name, nesting + 1);
            }
            cost
        }
        Expr::OperatorApplication {
            operator,
            left,
            right,
            ..
        } => {
            let mut cost = 0;
            // Boolean operators add complexity
            if operator == "&&" || operator == "||" {
                cost += 1;
            }
            cost += compute_complexity(left, func_name, nesting);
            cost += compute_complexity(right, func_name, nesting);
            cost
        }
        Expr::Application(args) => {
            let mut cost = 0;
            // Check for recursion (calling the same function name)
            if let Some(first) = args.first() {
                if let Expr::FunctionOrValue { module_name, name } = &first.value {
                    if module_name.is_empty() && name == func_name {
                        cost += 1; // recursion penalty
                    }
                }
            }
            for a in args {
                cost += compute_complexity(a, func_name, nesting);
            }
            cost
        }
        Expr::LetIn { declarations, body } => {
            let mut cost = 0;
            for decl in declarations {
                match &decl.value {
                    LetDeclaration::Function(f) => {
                        // Let functions add nesting
                        cost += compute_complexity(
                            &f.declaration.value.body,
                            func_name,
                            nesting + 1,
                        );
                    }
                    LetDeclaration::Destructuring { body: b, .. } => {
                        cost += compute_complexity(b, func_name, nesting);
                    }
                }
            }
            cost += compute_complexity(body, func_name, nesting);
            cost
        }
        Expr::Lambda { body, args, .. } => {
            // Lambdas increase nesting
            let mut cost = compute_complexity(body, func_name, nesting + 1);
            // Check if lambda args include patterns (adding implicit branching)
            for arg in args {
                if is_complex_pattern(&arg.value) {
                    cost += 1;
                }
            }
            cost
        }
        Expr::Parenthesized(inner) | Expr::Negation(inner) => {
            compute_complexity(inner, func_name, nesting)
        }
        Expr::Tuple(elems) | Expr::List(elems) => {
            let mut cost = 0;
            for e in elems {
                cost += compute_complexity(e, func_name, nesting);
            }
            cost
        }
        Expr::Record(fields) => {
            let mut cost = 0;
            for f in fields {
                cost += compute_complexity(&f.value.value, func_name, nesting);
            }
            cost
        }
        Expr::RecordUpdate { updates, .. } => {
            let mut cost = 0;
            for f in updates {
                cost += compute_complexity(&f.value.value, func_name, nesting);
            }
            cost
        }
        Expr::RecordAccess { record, .. } => compute_complexity(record, func_name, nesting),
        _ => 0,
    }
}

fn is_complex_pattern(pat: &Pattern) -> bool {
    matches!(
        pat,
        Pattern::Constructor { .. }
            | Pattern::Tuple(_)
            | Pattern::Record(_)
            | Pattern::Cons { .. }
            | Pattern::List(_)
            | Pattern::As { .. }
    )
}
