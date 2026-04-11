use elm_ast::exposing::Exposing;
use elm_ast::module_header::ModuleHeader;

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports `module Foo exposing (..)` — prefer explicit exposing lists.
pub struct NoExposingAll;

impl Rule for NoExposingAll {
    fn name(&self) -> &'static str {
        "NoExposingAll"
    }

    fn description(&self) -> &'static str {
        "Module headers should use an explicit exposing list instead of exposing (..)"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let exposing_node = match &ctx.module.header.value {
            ModuleHeader::Normal { exposing, .. }
            | ModuleHeader::Port { exposing, .. }
            | ModuleHeader::Effect { exposing, .. } => exposing,
        };

        if let Exposing::All(_) = &exposing_node.value {
            // Build an explicit exposing list from the module's declarations.
            let mut names: Vec<String> = Vec::new();
            for decl in &ctx.module.declarations {
                match &decl.value {
                    elm_ast::declaration::Declaration::FunctionDeclaration(func) => {
                        names.push(func.declaration.value.name.value.clone());
                    }
                    elm_ast::declaration::Declaration::AliasDeclaration(alias) => {
                        names.push(alias.name.value.clone());
                    }
                    elm_ast::declaration::Declaration::CustomTypeDeclaration(ct) => {
                        names.push(format!("{}(..)", ct.name.value));
                    }
                    elm_ast::declaration::Declaration::PortDeclaration(sig) => {
                        names.push(sig.name.value.clone());
                    }
                    elm_ast::declaration::Declaration::InfixDeclaration(_)
                    | elm_ast::declaration::Declaration::Destructuring { .. } => {}
                }
            }

            let fix = if names.is_empty() {
                None
            } else {
                let replacement = format!("({})", names.join(", "));
                Some(Fix::replace(exposing_node.span, replacement))
            };

            return vec![LintError {
                rule: self.name(),
                severity: Severity::Warning,
                message: "Module uses `exposing (..)` — prefer an explicit exposing list"
                    .to_string(),
                span: exposing_node.span,
                fix,
            }];
        }

        Vec::new()
    }
}
