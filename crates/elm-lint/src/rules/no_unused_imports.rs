use std::collections::HashSet;

use elm_ast::exposing::{ExposedItem, Exposing};
use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::pattern::Pattern;
use elm_ast::type_annotation::TypeAnnotation;
use elm_ast::visit::{self, Visit};

use crate::fix::remove_line;
use crate::rule::{Edit, Fix, LintContext, LintError, Rule, Severity};

pub struct NoUnusedImports;

impl Rule for NoUnusedImports {
    fn name(&self) -> &'static str {
        "NoUnusedImports"
    }

    fn description(&self) -> &'static str {
        "Reports imports that are never used"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut collector = RefCollector::default();
        collector.visit_module(ctx.module);

        let mut errors = Vec::new();

        for imp in &ctx.module.imports {
            let imp_module = imp.value.module_name.value.join(".");
            let imp_alias = imp
                .value
                .alias
                .as_ref()
                .map(|a| a.value.join("."))
                .unwrap_or_else(|| imp_module.clone());

            let qualified_used = collector
                .qualified_modules
                .iter()
                .any(|m| m == &imp_alias || m == &imp_module);

            let exposed_used = match &imp.value.exposing {
                None => false,
                Some(exp) => match &exp.value {
                    Exposing::All(_) => true,
                    Exposing::Explicit(items) => items.iter().any(|item| {
                        let name = exposed_item_name(&item.value);
                        collector.unqualified_refs.contains(&name)
                    }),
                },
            };

            if !qualified_used && !exposed_used {
                let remove_edit = Edit::Remove { span: imp.span };
                let line_edit = remove_line(ctx.source, &remove_edit);
                errors.push(LintError {
                    rule: self.name(),
                    severity: Severity::Warning,
                    message: format!("Import `{imp_module}` is not used"),
                    span: imp.span,
                    fix: Some(Fix {
                        edits: vec![line_edit],
                    }),
                });
            }
        }

        errors
    }
}

fn exposed_item_name(item: &ExposedItem) -> String {
    match item {
        ExposedItem::Function(n) => n.clone(),
        ExposedItem::TypeOrAlias(n) => n.clone(),
        ExposedItem::TypeExpose { name, .. } => name.clone(),
        ExposedItem::Infix(n) => n.clone(),
    }
}

#[derive(Default)]
struct RefCollector {
    qualified_modules: HashSet<String>,
    unqualified_refs: HashSet<String>,
}

impl Visit for RefCollector {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::FunctionOrValue { module_name, name } = &expr.value {
            if module_name.is_empty() {
                self.unqualified_refs.insert(name.clone());
            } else {
                self.qualified_modules.insert(module_name.join("."));
            }
        }
        visit::walk_expr(self, expr);
    }

    fn visit_pattern(&mut self, pattern: &Spanned<Pattern>) {
        if let Pattern::Constructor {
            module_name, name, ..
        } = &pattern.value
        {
            if module_name.is_empty() {
                self.unqualified_refs.insert(name.clone());
            } else {
                self.qualified_modules.insert(module_name.join("."));
            }
        }
        visit::walk_pattern(self, pattern);
    }

    fn visit_type_annotation(&mut self, ty: &Spanned<TypeAnnotation>) {
        if let TypeAnnotation::Typed {
            module_name, name, ..
        } = &ty.value
        {
            if module_name.is_empty() {
                self.unqualified_refs.insert(name.value.clone());
            } else {
                self.qualified_modules.insert(module_name.join("."));
            }
        }
        visit::walk_type_annotation(self, ty);
    }
}
