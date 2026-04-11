use elm_ast::declaration::Declaration;
use elm_ast::module_header::ModuleHeader;

use crate::rule::{Fix, LintContext, LintError, Rule, Severity};

/// Reports `port module` declarations when no ports are defined.
pub struct NoUnnecessaryPortModule;

impl Rule for NoUnnecessaryPortModule {
    fn name(&self) -> &'static str {
        "NoUnnecessaryPortModule"
    }

    fn description(&self) -> &'static str {
        "Modules declared as `port module` should contain at least one port declaration"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        // Only applies to port modules.
        let (name_node, exposing_node) = match &ctx.module.header.value {
            ModuleHeader::Port { name, exposing } => (name, exposing),
            _ => return Vec::new(),
        };

        // Check if any declaration is a port.
        let has_ports = ctx
            .module
            .declarations
            .iter()
            .any(|d| matches!(&d.value, Declaration::PortDeclaration(_)));

        if has_ports {
            return Vec::new();
        }

        // Fix: replace "port module Foo exposing (..)" with "module Foo exposing (..)"
        let header_text =
            &ctx.source[ctx.module.header.span.start.offset..ctx.module.header.span.end.offset];

        let fix = if let Some(port_start) = header_text.find("port") {
            let module_start = header_text[port_start..].find("module");
            if let Some(mod_offset) = module_start {
                // Reconstruct without "port "
                let name_text = &ctx.source[name_node.span.start.offset..name_node.span.end.offset];
                let exposing_text =
                    &ctx.source[exposing_node.span.start.offset..exposing_node.span.end.offset];
                let _ = mod_offset; // used for verification only
                Some(Fix::replace(
                    ctx.module.header.span,
                    format!("module {name_text} exposing {exposing_text}"),
                ))
            } else {
                None
            }
        } else {
            None
        };

        vec![LintError {
            rule: self.name(),
            severity: Severity::Warning,
            message: "Module is declared as `port module` but contains no port declarations"
                .to_string(),
            span: ctx.module.header.span,
            fix,
        }]
    }
}
