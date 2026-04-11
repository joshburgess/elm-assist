use elm_ast::declaration::Declaration;

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports port declarations whose name collides with a port in another module.
/// Duplicate port names cause runtime errors in Elm.
pub struct NoDuplicatePorts;

impl Rule for NoDuplicatePorts {
    fn name(&self) -> &'static str {
        "NoDuplicatePorts"
    }

    fn description(&self) -> &'static str {
        "Port names must be unique across all modules in the project"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let Some(project) = ctx.project else {
            return Vec::new();
        };

        let module_name = ctx
            .module_info
            .map(|info| info.module_name.join("."))
            .unwrap_or_default();

        let mut errors = Vec::new();

        // Collect port names from this module.
        let local_ports: Vec<_> = ctx
            .module
            .declarations
            .iter()
            .filter_map(|decl| {
                if let Declaration::PortDeclaration(sig) = &decl.value {
                    Some(sig)
                } else {
                    None
                }
            })
            .collect();

        if local_ports.is_empty() {
            return Vec::new();
        }

        // Check each local port against other modules' defined_ports.
        for sig in &local_ports {
            let port_name = &sig.name.value;

            for (other_mod, other_info) in &project.modules {
                if *other_mod == module_name {
                    continue;
                }
                if other_info.defined_ports.contains(port_name.as_str()) {
                    errors.push(LintError {
                        rule: "NoDuplicatePorts",
                        severity: Severity::Error,
                        message: format!(
                            "Port `{port_name}` is also declared in module `{other_mod}`"
                        ),
                        span: sig.name.span,
                        fix: None,
                    });
                }
            }
        }

        errors
    }
}
