use std::collections::HashMap;

use elm_ast::declaration::Declaration;
use elm_ast::expr::CaseBranch;
use elm_ast::node::Spanned;
use elm_ast::pattern::Pattern;
use elm_ast::visit::{self, Visit};

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports custom type constructor arguments that are always ignored with `_` in case branches.
pub struct NoUnusedCustomTypeConstructorArgs;

impl Rule for NoUnusedCustomTypeConstructorArgs {
    fn name(&self) -> &'static str {
        "NoUnusedCustomTypeConstructorArgs"
    }

    fn description(&self) -> &'static str {
        "Constructor arguments that are always ignored with _ across all case branches may be unnecessary"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        // Collect constructors with their argument counts from type definitions.
        let mut ctor_arg_counts: HashMap<String, usize> = HashMap::new();
        for decl in &ctx.module.declarations {
            if let Declaration::CustomTypeDeclaration(ct) = &decl.value {
                for ctor in &ct.constructors {
                    ctor_arg_counts.insert(ctor.value.name.value.clone(), ctor.value.args.len());
                }
            }
        }

        if ctor_arg_counts.is_empty() {
            return Vec::new();
        }

        // Visit case expressions and collect pattern usage per constructor.
        let mut visitor = Visitor {
            ctor_arg_counts: &ctor_arg_counts,
            // Map: (ctor_name, arg_index) -> (times_seen, times_wildcard)
            arg_usage: HashMap::new(),
            errors: Vec::new(),
        };
        visitor.visit_module(ctx.module);

        // Report constructors where an argument is always `_`.
        for ((ctor_name, arg_idx), (seen, wildcarded)) in &visitor.arg_usage {
            if *seen >= 1 && seen == wildcarded {
                visitor.errors.push(LintError {
                    rule: "NoUnusedCustomTypeConstructorArgs",
                    severity: Severity::Warning,
                    message: format!(
                        "Argument {} of `{ctor_name}` is always ignored with `_`",
                        arg_idx + 1
                    ),
                    // Use the span of the constructor type definition.
                    span: find_ctor_span(ctx.module, ctor_name).unwrap_or(ctx.module.header.span),
                    fix: None,
                });
            }
        }

        visitor.errors
    }
}

struct Visitor<'a> {
    ctor_arg_counts: &'a HashMap<String, usize>,
    arg_usage: HashMap<(String, usize), (usize, usize)>,
    errors: Vec<LintError>,
}

impl Visit for Visitor<'_> {
    fn visit_case_branch(&mut self, branch: &CaseBranch) {
        self.check_pattern(&branch.pattern);
        visit::walk_case_branch(self, branch);
    }
}

impl Visitor<'_> {
    fn check_pattern(&mut self, pattern: &Spanned<Pattern>) {
        match &pattern.value {
            Pattern::Constructor {
                module_name,
                name,
                args,
            } => {
                if !module_name.is_empty() {
                    return; // Skip qualified constructors (from other modules).
                }
                if let Some(&expected_args) = self.ctor_arg_counts.get(name) {
                    if args.len() == expected_args {
                        for (i, arg) in args.iter().enumerate() {
                            let entry = self.arg_usage.entry((name.clone(), i)).or_insert((0, 0));
                            entry.0 += 1;
                            if is_wildcard_pattern(&arg.value) {
                                entry.1 += 1;
                            }
                        }
                    }
                }
            }
            Pattern::Parenthesized(inner) => self.check_pattern(inner),
            Pattern::As { pattern: inner, .. } => self.check_pattern(inner),
            _ => {}
        }
    }
}

fn is_wildcard_pattern(pat: &Pattern) -> bool {
    match pat {
        Pattern::Anything => true,
        Pattern::Parenthesized(inner) => is_wildcard_pattern(&inner.value),
        _ => false,
    }
}

fn find_ctor_span(
    module: &elm_ast::file::ElmModule,
    ctor_name: &str,
) -> Option<elm_ast::span::Span> {
    for decl in &module.declarations {
        if let Declaration::CustomTypeDeclaration(ct) = &decl.value {
            for ctor in &ct.constructors {
                if ctor.value.name.value == ctor_name {
                    return Some(ctor.value.name.span);
                }
            }
        }
    }
    None
}
