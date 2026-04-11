use crate::rule::{LintContext, LintError, Rule, Severity};

pub struct NoUnusedModules;

impl Rule for NoUnusedModules {
    fn name(&self) -> &'static str {
        "NoUnusedModules"
    }

    fn description(&self) -> &'static str {
        "Modules that are never imported by any other module"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let (Some(info), Some(project)) = (ctx.module_info, ctx.project) else {
            return Vec::new();
        };

        let mod_name = info.module_name.join(".");

        // Don't flag Main modules — they're entry points.
        if mod_name == "Main" {
            return Vec::new();
        }

        let is_imported = project
            .importers
            .get(&mod_name)
            .is_some_and(|importers| !importers.is_empty());

        if !is_imported {
            vec![LintError {
                rule: "NoUnusedModules",
                    severity: Severity::Warning,
                message: format!("Module `{mod_name}` is never imported by any other module"),
                span: ctx.module.header.span,
                fix: None,
            }]
        } else {
            Vec::new()
        }
    }
}
