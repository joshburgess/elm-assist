use elm_ast::declaration::Declaration;

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports top-level functions missing type annotations.
pub struct NoMissingTypeAnnotation;

impl Rule for NoMissingTypeAnnotation {
    fn name(&self) -> &'static str {
        "NoMissingTypeAnnotation"
    }

    fn description(&self) -> &'static str {
        "Top-level function definitions should have type annotations"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut errors = Vec::new();

        for decl in &ctx.module.declarations {
            if let Declaration::FunctionDeclaration(func) = &decl.value {
                if func.signature.is_none() {
                    let name = &func.declaration.value.name.value;
                    errors.push(LintError {
                        rule: self.name(),
                    severity: Severity::Warning,
                        message: format!("`{name}` is missing a type annotation"),
                        span: func.declaration.value.name.span,
                        fix: None,
                    });
                }
            }
        }

        errors
    }
}
