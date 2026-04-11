use elm_ast::comment::Comment;
use elm_ast::declaration::Declaration;
use elm_ast::exposing::{ExposedItem, Exposing};
use elm_ast::module_header::ModuleHeader;
use elm_ast::node::Spanned;

use crate::rule::{LintContext, LintError, Rule, Severity};

/// Reports public functions and types that have no doc comment.
pub struct NoMissingDocumentation;

impl Rule for NoMissingDocumentation {
    fn name(&self) -> &'static str {
        "NoMissingDocumentation"
    }

    fn description(&self) -> &'static str {
        "Exposed functions and types should have documentation comments"
    }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let exposing_node = match &ctx.module.header.value {
            ModuleHeader::Normal { exposing, .. }
            | ModuleHeader::Port { exposing, .. }
            | ModuleHeader::Effect { exposing, .. } => exposing,
        };

        // If exposing (..), everything is public.
        let (exposes_all, exposed_names) = match &exposing_node.value {
            Exposing::All(_) => (true, Vec::new()),
            Exposing::Explicit(items) => {
                let names: Vec<String> = items
                    .iter()
                    .map(|item| match &item.value {
                        ExposedItem::Function(n) => n.clone(),
                        ExposedItem::TypeOrAlias(n) => n.clone(),
                        ExposedItem::TypeExpose { name, .. } => name.clone(),
                        ExposedItem::Infix(n) => n.clone(),
                    })
                    .collect();
                (false, names)
            }
        };

        let mut errors = Vec::new();

        for decl in &ctx.module.declarations {
            match &decl.value {
                Declaration::FunctionDeclaration(func) => {
                    let name = &func.declaration.value.name.value;
                    if is_exposed(name, exposes_all, &exposed_names)
                        && func.documentation.is_none()
                        && !has_doc_comment(&decl.comments)
                    {
                        errors.push(LintError {
                            rule: self.name(),
                            severity: Severity::Warning,
                            message: format!(
                                "`{name}` is exposed but has no documentation comment"
                            ),
                            span: func.declaration.value.name.span,
                            fix: None,
                        });
                    }
                }
                Declaration::AliasDeclaration(alias) => {
                    let name = &alias.name.value;
                    if is_exposed(name, exposes_all, &exposed_names)
                        && alias.documentation.is_none()
                        && !has_doc_comment(&decl.comments)
                    {
                        errors.push(LintError {
                            rule: self.name(),
                            severity: Severity::Warning,
                            message: format!(
                                "Type alias `{name}` is exposed but has no documentation comment"
                            ),
                            span: alias.name.span,
                            fix: None,
                        });
                    }
                }
                Declaration::CustomTypeDeclaration(ct) => {
                    let name = &ct.name.value;
                    if is_exposed(name, exposes_all, &exposed_names)
                        && ct.documentation.is_none()
                        && !has_doc_comment(&decl.comments)
                    {
                        errors.push(LintError {
                            rule: self.name(),
                            severity: Severity::Warning,
                            message: format!(
                                "Type `{name}` is exposed but has no documentation comment"
                            ),
                            span: ct.name.span,
                            fix: None,
                        });
                    }
                }
                Declaration::PortDeclaration(_)
                | Declaration::InfixDeclaration(_)
                | Declaration::Destructuring { .. } => {}
            }
        }

        errors
    }
}

fn is_exposed(name: &str, exposes_all: bool, exposed_names: &[String]) -> bool {
    exposes_all || exposed_names.iter().any(|n| n == name)
}

/// Check if any leading comment is a doc comment (`{-| ... -}`).
fn has_doc_comment(comments: &[Spanned<Comment>]) -> bool {
    comments.iter().any(|c| matches!(&c.value, Comment::Doc(_)))
}
