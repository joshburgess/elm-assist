use elm_ast::exposing::Exposing;

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports `import Foo exposing (..)` — prefer explicit import lists.
pub struct NoImportExposingAll;

impl Rule for NoImportExposingAll {
    fn name(&self) -> &'static str {
        "NoImportExposingAll"
    }

    fn description(&self) -> &'static str {
        "Import statements should use an explicit exposing list instead of exposing (..)"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut errors = Vec::new();

        for imp in &ctx.module.imports {
            if let Some(exposing_node) = &imp.value.exposing {
                if let Exposing::All(_) = &exposing_node.value {
                    let imp_module = imp.value.module_name.value.join(".");

                    // We can offer a fix to remove the exposing clause entirely,
                    // leaving qualified access. Computing the exact used names
                    // would require cross-module info we may not have.
                    let fix = Some(Fix::replace(exposing_node.span, String::new()));

                    errors.push(LintError {
                        rule: self.name(),
                        severity: Severity::Warning,
                        message: format!(
                            "Import `{imp_module}` uses `exposing (..)` — prefer an explicit list"
                        ),
                        span: exposing_node.span,
                        fix,
                    });
                }
            }
        }

        errors
    }
}
