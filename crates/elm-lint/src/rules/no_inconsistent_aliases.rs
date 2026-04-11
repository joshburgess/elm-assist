use std::collections::HashMap;

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports imports whose alias doesn't match the project's canonical alias.
///
/// Configure canonical aliases in `elm-assist.toml`:
///
/// ```toml
/// [rules.NoInconsistentAliases]
/// aliases = { "Json.Decode" = "Decode", "Json.Encode" = "Encode", "Html.Attributes" = "Attr" }
/// ```
///
/// Without configuration, this rule does nothing — all aliases are accepted.
#[derive(Default)]
pub struct NoInconsistentAliases {
    /// Canonical aliases: module name -> expected alias.
    canonical: HashMap<String, String>,
}

impl Rule for NoInconsistentAliases {
    fn name(&self) -> &'static str {
        "NoInconsistentAliases"
    }

    fn description(&self) -> &'static str {
        "Import aliases should be consistent with the project's canonical aliases"
    }

    fn configure(&mut self, options: &toml::Value) -> Result<(), String> {
        if let Some(aliases) = options.get("aliases") {
            let table = aliases.as_table().ok_or_else(|| {
                "aliases must be a table (e.g. { \"Json.Decode\" = \"Decode\" })".to_string()
            })?;
            for (module, alias_val) in table {
                let alias = alias_val
                    .as_str()
                    .ok_or_else(|| format!("alias for `{module}` must be a string"))?;
                self.canonical.insert(module.clone(), alias.to_string());
            }
        }
        Ok(())
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        if self.canonical.is_empty() {
            return Vec::new();
        }

        let mut errors = Vec::new();

        for imp in &ctx.module.imports {
            let module_name = imp.value.module_name.value.join(".");

            if let Some(expected_alias) = self.canonical.get(&module_name) {
                // This module has a canonical alias — check if the import uses it.
                let actual_alias = imp.value.alias.as_ref().map(|a| a.value.join("."));

                let matches = match &actual_alias {
                    Some(alias) => alias == expected_alias,
                    None => {
                        // No alias — the default alias is the last segment of the module name.
                        let default = imp
                            .value
                            .module_name
                            .value
                            .last()
                            .cloned()
                            .unwrap_or_default();
                        &default == expected_alias
                    }
                };

                if !matches {
                    let actual_display = actual_alias.as_deref().unwrap_or(&module_name);

                    errors.push(LintError {
                        rule: "NoInconsistentAliases",
                        severity: Severity::Warning,
                        message: format!(
                            "`{module_name}` should be aliased as `{expected_alias}`, not `{actual_display}`"
                        ),
                        span: imp.value.module_name.span,
                        fix: None,
                    });
                }
            }
        }

        errors
    }
}
