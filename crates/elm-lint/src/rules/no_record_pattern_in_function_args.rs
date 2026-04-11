use elm_ast::declaration::Declaration;
use elm_ast::pattern::Pattern;

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports record destructuring patterns in function arguments.
/// Prefer binding to a name and using `record.field` access for clarity.
pub struct NoRecordPatternInFunctionArgs;

impl Rule for NoRecordPatternInFunctionArgs {
    fn name(&self) -> &'static str {
        "NoRecordPatternInFunctionArgs"
    }

    fn description(&self) -> &'static str {
        "Function arguments should not use record destructuring patterns — use record.field access instead"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut errors = Vec::new();

        for decl in &ctx.module.declarations {
            if let Declaration::FunctionDeclaration(func) = &decl.value {
                for arg in &func.declaration.value.args {
                    check_pattern(arg.span, &arg.value, &mut errors);
                }
            }
        }

        errors
    }
}

fn check_pattern(span: elm_ast::span::Span, pat: &Pattern, errors: &mut Vec<LintError>) {
    match pat {
        Pattern::Record(fields) => {
            let field_names: Vec<&str> = fields.iter().map(|f| f.value.as_str()).collect();
            errors.push(LintError {
                rule: "NoRecordPatternInFunctionArgs",
                severity: Severity::Warning,
                message: format!(
                    "Record pattern `{{ {} }}` in function argument — prefer a named parameter with `.field` access",
                    field_names.join(", ")
                ),
                span,
                fix: None,
            });
        }
        Pattern::Parenthesized(inner) => {
            check_pattern(span, &inner.value, errors);
        }
        _ => {}
    }
}
