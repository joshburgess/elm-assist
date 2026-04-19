use std::collections::HashSet;

use elm_ast::declaration::Declaration;
use elm_ast::exposing::{ExposedItem, Exposing};
use elm_ast::module_header::ModuleHeader;
use elm_ast::node::Spanned;
use elm_ast::type_annotation::TypeAnnotation;

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports types used in exposed function signatures that are not themselves exposed.
///
/// If a module exposes `foo : MyType -> Int` but doesn't expose `MyType`, consumers
/// can't write type annotations using that function's types.
pub struct NoMissingTypeExpose;

impl Rule for NoMissingTypeExpose {
    fn name(&self) -> &'static str {
        "NoMissingTypeExpose"
    }

    fn description(&self) -> &'static str {
        "Types used in exposed function signatures should also be exposed"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let exposing_node = match &ctx.module.header.value {
            ModuleHeader::Normal { exposing, .. }
            | ModuleHeader::Port { exposing, .. }
            | ModuleHeader::Effect { exposing, .. } => exposing,
        };

        // If exposing (..), everything is already exposed.
        let exposed_names: HashSet<String> = match &exposing_node.value {
            Exposing::All(_) => return Vec::new(),
            Exposing::Explicit { items, .. } => items
                .iter()
                .map(|item| match &item.value {
                    ExposedItem::Function(n) => n.clone(),
                    ExposedItem::TypeOrAlias(n) => n.clone(),
                    ExposedItem::TypeExpose { name, .. } => name.clone(),
                    ExposedItem::Infix(n) => n.clone(),
                })
                .collect(),
        };

        // Collect types defined in this module.
        let mut defined_types: HashSet<String> = HashSet::new();
        for decl in &ctx.module.declarations {
            match &decl.value {
                Declaration::AliasDeclaration(alias) => {
                    defined_types.insert(alias.name.value.clone());
                }
                Declaration::CustomTypeDeclaration(ct) => {
                    defined_types.insert(ct.name.value.clone());
                }
                _ => {}
            }
        }

        let mut errors = Vec::new();

        // For each exposed function, check its type signature for references to
        // locally-defined types that aren't exposed.
        for decl in &ctx.module.declarations {
            if let Declaration::FunctionDeclaration(func) = &decl.value {
                let name = &func.declaration.value.name.value;
                if !exposed_names.contains(name) {
                    continue;
                }

                if let Some(sig) = &func.signature {
                    let mut referenced = HashSet::new();
                    collect_type_refs(&sig.value.type_annotation, &mut referenced);

                    for type_name in &referenced {
                        if defined_types.contains(type_name) && !exposed_names.contains(type_name) {
                            errors.push(LintError {
                                rule: self.name(),
                                severity: Severity::Warning,
                                message: format!(
                                    "Type `{type_name}` is used in `{name}`'s signature but is not exposed"
                                ),
                                span: sig.span,
                                fix: None,
                            });
                        }
                    }
                }
            }

            // Also check port declarations.
            if let Declaration::PortDeclaration(sig) = &decl.value {
                let name = &sig.name.value;
                if !exposed_names.contains(name) {
                    continue;
                }

                let mut referenced = HashSet::new();
                collect_type_refs(&sig.type_annotation, &mut referenced);

                for type_name in &referenced {
                    if defined_types.contains(type_name) && !exposed_names.contains(type_name) {
                        errors.push(LintError {
                            rule: self.name(),
                            severity: Severity::Warning,
                            message: format!(
                                "Type `{type_name}` is used in `{name}`'s signature but is not exposed"
                            ),
                            span: sig.type_annotation.span,
                            fix: None,
                        });
                    }
                }
            }
        }

        errors
    }
}

fn collect_type_refs(ty: &Spanned<TypeAnnotation>, out: &mut HashSet<String>) {
    match &ty.value {
        TypeAnnotation::Typed {
            module_name,
            name,
            args,
        } => {
            // Only local (unqualified) types matter.
            if module_name.is_empty() {
                out.insert(name.value.clone());
            }
            for arg in args {
                collect_type_refs(arg, out);
            }
        }
        TypeAnnotation::FunctionType { from, to } => {
            collect_type_refs(from, out);
            collect_type_refs(to, out);
        }
        TypeAnnotation::Tupled(elems) => {
            for e in elems {
                collect_type_refs(e, out);
            }
        }
        TypeAnnotation::Record(fields) | TypeAnnotation::GenericRecord { fields, .. } => {
            for f in fields {
                collect_type_refs(&f.value.type_annotation, out);
            }
        }
        TypeAnnotation::Unit | TypeAnnotation::GenericType(_) => {}
    }
}
