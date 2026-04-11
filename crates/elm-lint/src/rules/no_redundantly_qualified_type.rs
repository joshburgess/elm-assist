use elm_ast::node::Spanned;
use elm_ast::type_annotation::TypeAnnotation;
use elm_ast::visit::{self, Visit};

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports redundantly qualified types like `Set.Set` or `Dict.Dict` where the
/// type name matches the module or alias name.
pub struct NoRedundantlyQualifiedType;

impl Rule for NoRedundantlyQualifiedType {
    fn name(&self) -> &'static str {
        "NoRedundantlyQualifiedType"
    }

    fn description(&self) -> &'static str {
        "Types should not be redundantly qualified when the type name matches the module name"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        // Build a map of import aliases for this module.
        let mut aliases: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for imp in &ctx.module.imports {
            let module_name = imp.value.module_name.value.join(".");
            let alias = imp
                .value
                .alias
                .as_ref()
                .map(|a| a.value.join("."))
                .unwrap_or_else(|| {
                    // Default alias is the last segment of the module name.
                    imp.value
                        .module_name
                        .value
                        .last()
                        .cloned()
                        .unwrap_or_default()
                });
            aliases.insert(alias, module_name);
        }

        let _ = aliases; // Available for future use with aliased imports.

        let mut visitor = Visitor {
            source: ctx.source,
            errors: Vec::new(),
        };
        visitor.visit_module(ctx.module);
        visitor.errors
    }
}

struct Visitor<'a> {
    source: &'a str,
    errors: Vec<LintError>,
}

impl Visit for Visitor<'_> {
    fn visit_type_annotation(&mut self, ty: &Spanned<TypeAnnotation>) {
        if let TypeAnnotation::Typed {
            module_name, name, ..
        } = &ty.value
        {
            if !module_name.is_empty() {
                let qualifier = module_name.join(".");
                // Check if the last segment of the qualifier matches the type name.
                let last_segment = module_name.last().map(|s| s.as_str()).unwrap_or("");
                if last_segment == name.value {
                    // This is a redundantly qualified type like Set.Set or Dict.Dict.
                    // Fix: replace the full qualified reference with just the type name.
                    let full_text = &self.source[ty.span.start.offset..ty.span.end.offset];
                    // Find where the type args start (if any) to only replace the qualified name part.
                    let qualified_name = format!("{}.{}", qualifier, name.value);
                    if full_text.starts_with(&qualified_name) {
                        let replacement_end = ty.span.start.offset + qualified_name.len();
                        let replace_span = elm_ast::span::Span {
                            start: ty.span.start,
                            end: elm_ast::span::Position {
                                offset: replacement_end,
                                line: name.span.end.line,
                                column: name.span.end.column,
                            },
                        };
                        self.errors.push(LintError {
                            rule: "NoRedundantlyQualifiedType",
                            severity: Severity::Warning,
                            message: format!(
                                "`{qualifier}.{0}` is redundantly qualified — use `{0}` instead",
                                name.value
                            ),
                            span: ty.span,
                            fix: Some(Fix::replace(replace_span, name.value.clone())),
                        });
                    }
                }
            }
        }
        visit::walk_type_annotation(self, ty);
    }
}
