use std::collections::HashSet;

use elm_ast::declaration::Declaration;
use elm_ast::expr::Expr;
use elm_ast::file::ElmModule;
use elm_ast::node::Spanned;
use elm_ast::pattern::Pattern;
use elm_ast::span::Span;
use elm_ast::type_annotation::TypeAnnotation;
use elm_ast::visit::{self, Visit};

use crate::query::{ExprKindQuery, Query};

/// A single search match.
#[derive(Debug)]
pub struct Match {
    pub span: Span,
    pub context: String, // description of what matched
}

/// Run a query against a parsed module.
pub fn search(module: &ElmModule, query: &Query) -> Vec<Match> {
    match query {
        Query::ReturnsType(type_name) => search_returns_type(module, type_name),
        Query::UsesType(type_name) => search_uses_type(module, type_name),
        Query::CaseOn(name) => search_case_on(module, name),
        Query::RecordUpdateField(field) => search_record_update(module, field),
        Query::CallsTo(module_name) => search_calls_to(module, module_name),
        Query::UnusedArgs => search_unused_args(module),
        Query::LambdaArity(n) => search_lambda_arity(module, *n),
        Query::Uses(name) => search_uses(module, name),
        Query::Defined(pattern) => search_defined(module, pattern),
        Query::ExprKind(kind) => search_expr_kind(module, kind),
    }
}

// ── Query implementations ────────────────────────────────────────────

fn search_returns_type(module: &ElmModule, type_name: &str) -> Vec<Match> {
    let mut matches = Vec::new();
    for decl in &module.declarations {
        if let Declaration::FunctionDeclaration(func) = &decl.value
            && let Some(sig) = &func.signature
        {
            let return_type = get_return_type(&sig.value.type_annotation.value);
            if type_contains(return_type, type_name) {
                matches.push(Match {
                    span: sig.value.name.span,
                    context: format!("{} : ... -> {}", sig.value.name.value, type_name),
                });
            }
        }
    }
    matches
}

fn search_uses_type(module: &ElmModule, type_name: &str) -> Vec<Match> {
    let mut matches = Vec::new();
    for decl in &module.declarations {
        if let Declaration::FunctionDeclaration(func) = &decl.value
            && let Some(sig) = &func.signature
            && type_contains(&sig.value.type_annotation.value, type_name)
        {
            matches.push(Match {
                span: sig.value.name.span,
                context: format!("{} uses type {type_name}", sig.value.name.value),
            });
        }
    }
    matches
}

fn search_case_on(module: &ElmModule, name: &str) -> Vec<Match> {
    let mut visitor = CaseOnVisitor {
        name: name.to_string(),
        matches: Vec::new(),
    };
    visitor.visit_module(module);
    visitor.matches
}

struct CaseOnVisitor {
    name: String,
    matches: Vec<Match>,
}

impl Visit for CaseOnVisitor {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::CaseOf { branches, .. } = &expr.value {
            for branch in branches {
                if pattern_mentions_constructor(&branch.pattern.value, &self.name) {
                    self.matches.push(Match {
                        span: expr.span,
                        context: format!("case matching on {}", self.name),
                    });
                    break;
                }
            }
        }
        visit::walk_expr(self, expr);
    }
}

fn search_record_update(module: &ElmModule, field: &str) -> Vec<Match> {
    let mut visitor = RecordUpdateVisitor {
        field: field.to_string(),
        matches: Vec::new(),
    };
    visitor.visit_module(module);
    visitor.matches
}

struct RecordUpdateVisitor {
    field: String,
    matches: Vec<Match>,
}

impl Visit for RecordUpdateVisitor {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::RecordUpdate { base, updates } = &expr.value {
            for update in updates {
                if update.value.field.value == self.field {
                    self.matches.push(Match {
                        span: expr.span,
                        context: format!("{{ {} | {} = ... }}", base.value, self.field),
                    });
                    break;
                }
            }
        }
        visit::walk_expr(self, expr);
    }
}

fn search_calls_to(module: &ElmModule, target_module: &str) -> Vec<Match> {
    let mut visitor = CallsToVisitor {
        module_name: target_module.to_string(),
        matches: Vec::new(),
    };
    visitor.visit_module(module);
    visitor.matches
}

struct CallsToVisitor {
    module_name: String,
    matches: Vec<Match>,
}

impl Visit for CallsToVisitor {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::FunctionOrValue { module_name, name } = &expr.value {
            let qualified = module_name.join(".");
            if qualified == self.module_name
                || module_name
                    .first()
                    .is_some_and(|first| first == &self.module_name)
            {
                self.matches.push(Match {
                    span: expr.span,
                    context: format!("{qualified}.{name}"),
                });
            }
        }
        visit::walk_expr(self, expr);
    }
}

fn search_unused_args(module: &ElmModule) -> Vec<Match> {
    let mut matches = Vec::new();
    for decl in &module.declarations {
        if let Declaration::FunctionDeclaration(func) = &decl.value {
            let imp = &func.declaration.value;
            if imp.args.is_empty() {
                continue;
            }

            // Collect all variable names used in the body.
            let mut used = HashSet::new();
            collect_used_names(&imp.body.value, &mut used);

            // Check each argument.
            for arg in &imp.args {
                let arg_names = pattern_var_names(&arg.value);
                for name in arg_names {
                    if name != "_" && !used.contains(&name) {
                        matches.push(Match {
                            span: arg.span,
                            context: format!("{}: argument `{name}` is never used", imp.name.value),
                        });
                    }
                }
            }
        }
    }
    matches
}

fn search_lambda_arity(module: &ElmModule, min_arity: usize) -> Vec<Match> {
    let mut visitor = LambdaArityVisitor {
        min_arity,
        matches: Vec::new(),
    };
    visitor.visit_module(module);
    visitor.matches
}

struct LambdaArityVisitor {
    min_arity: usize,
    matches: Vec<Match>,
}

impl Visit for LambdaArityVisitor {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::Lambda { args, .. } = &expr.value
            && args.len() >= self.min_arity
        {
            self.matches.push(Match {
                span: expr.span,
                context: format!("lambda with {} args", args.len()),
            });
        }
        visit::walk_expr(self, expr);
    }
}

fn search_uses(module: &ElmModule, name: &str) -> Vec<Match> {
    let mut visitor = UsesVisitor {
        name: name.to_string(),
        matches: Vec::new(),
    };
    visitor.visit_module(module);
    visitor.matches
}

struct UsesVisitor {
    name: String,
    matches: Vec<Match>,
}

impl Visit for UsesVisitor {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::FunctionOrValue { module_name, name } = &expr.value
            && name == &self.name
        {
            let full = if module_name.is_empty() {
                name.clone()
            } else {
                format!("{}.{name}", module_name.join("."))
            };
            self.matches.push(Match {
                span: expr.span,
                context: full,
            });
        }
        visit::walk_expr(self, expr);
    }
}

fn search_defined(module: &ElmModule, pattern: &str) -> Vec<Match> {
    let mut matches = Vec::new();
    for decl in &module.declarations {
        match &decl.value {
            Declaration::FunctionDeclaration(func) => {
                let name = &func.declaration.value.name.value;
                if name.contains(pattern) {
                    matches.push(Match {
                        span: func.declaration.value.name.span,
                        context: format!("function {name}"),
                    });
                }
            }
            Declaration::CustomTypeDeclaration(ct) => {
                if ct.name.value.contains(pattern) {
                    matches.push(Match {
                        span: ct.name.span,
                        context: format!("type {}", ct.name.value),
                    });
                }
            }
            Declaration::AliasDeclaration(alias) => {
                if alias.name.value.contains(pattern) {
                    matches.push(Match {
                        span: alias.name.span,
                        context: format!("type alias {}", alias.name.value),
                    });
                }
            }
            _ => {}
        }
    }
    matches
}

fn search_expr_kind(module: &ElmModule, kind: &ExprKindQuery) -> Vec<Match> {
    let mut visitor = ExprKindVisitor {
        kind: kind.clone(),
        matches: Vec::new(),
    };
    visitor.visit_module(module);
    visitor.matches
}

struct ExprKindVisitor {
    kind: ExprKindQuery,
    matches: Vec<Match>,
}

impl Visit for ExprKindVisitor {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        let matched = match (&self.kind, &expr.value) {
            (ExprKindQuery::Let, Expr::LetIn { .. }) => Some("let expression"),
            (ExprKindQuery::Case, Expr::CaseOf { .. }) => Some("case expression"),
            (ExprKindQuery::If, Expr::IfElse { .. }) => Some("if expression"),
            (ExprKindQuery::Lambda, Expr::Lambda { .. }) => Some("lambda"),
            (ExprKindQuery::Record, Expr::Record(_)) => Some("record"),
            (ExprKindQuery::List, Expr::List { .. }) => Some("list"),
            (ExprKindQuery::Tuple, Expr::Tuple(_)) => Some("tuple"),
            _ => None,
        };
        if let Some(desc) = matched {
            self.matches.push(Match {
                span: expr.span,
                context: desc.to_string(),
            });
        }
        visit::walk_expr(self, expr);
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn get_return_type(ty: &TypeAnnotation) -> &TypeAnnotation {
    match ty {
        TypeAnnotation::FunctionType { to, .. } => get_return_type(&to.value),
        other => other,
    }
}

fn type_contains(ty: &TypeAnnotation, name: &str) -> bool {
    match ty {
        TypeAnnotation::Typed {
            name: type_name,
            args,
            ..
        } => type_name.value == name || args.iter().any(|a| type_contains(&a.value, name)),
        TypeAnnotation::FunctionType { from, to } => {
            type_contains(&from.value, name) || type_contains(&to.value, name)
        }
        TypeAnnotation::Tupled(elems) => elems.iter().any(|e| type_contains(&e.value, name)),
        TypeAnnotation::Record(fields) => fields
            .iter()
            .any(|f| type_contains(&f.value.type_annotation.value, name)),
        TypeAnnotation::GenericRecord { fields, .. } => fields
            .iter()
            .any(|f| type_contains(&f.value.type_annotation.value, name)),
        _ => false,
    }
}

fn pattern_mentions_constructor(pat: &Pattern, name: &str) -> bool {
    match pat {
        Pattern::Constructor {
            name: ctor_name,
            args,
            ..
        } => {
            ctor_name == name
                || args
                    .iter()
                    .any(|a| pattern_mentions_constructor(&a.value, name))
        }
        Pattern::Tuple(elems) | Pattern::List(elems) => elems
            .iter()
            .any(|e| pattern_mentions_constructor(&e.value, name)),
        Pattern::Cons { head, tail } => {
            pattern_mentions_constructor(&head.value, name)
                || pattern_mentions_constructor(&tail.value, name)
        }
        Pattern::As { pattern: inner, .. } | Pattern::Parenthesized(inner) => {
            pattern_mentions_constructor(&inner.value, name)
        }
        _ => false,
    }
}

fn pattern_var_names(pat: &Pattern) -> Vec<String> {
    match pat {
        Pattern::Var(name) => vec![name.clone()],
        Pattern::Anything => vec!["_".into()],
        Pattern::Tuple(elems) | Pattern::List(elems) => elems
            .iter()
            .flat_map(|e| pattern_var_names(&e.value))
            .collect(),
        Pattern::Constructor { args, .. } => args
            .iter()
            .flat_map(|a| pattern_var_names(&a.value))
            .collect(),
        Pattern::Record(fields) => fields.iter().map(|f| f.value.clone()).collect(),
        Pattern::Cons { head, tail } => {
            let mut names = pattern_var_names(&head.value);
            names.extend(pattern_var_names(&tail.value));
            names
        }
        Pattern::As {
            pattern: inner,
            name,
        } => {
            let mut names = pattern_var_names(&inner.value);
            names.push(name.value.clone());
            names
        }
        Pattern::Parenthesized(inner) => pattern_var_names(&inner.value),
        _ => Vec::new(),
    }
}

fn collect_used_names(expr: &Expr, names: &mut HashSet<String>) {
    match expr {
        Expr::FunctionOrValue { module_name, name } if module_name.is_empty() => {
            names.insert(name.clone());
        }
        Expr::Application(args) => {
            for a in args {
                collect_used_names(&a.value, names);
            }
        }
        Expr::OperatorApplication { left, right, .. } => {
            collect_used_names(&left.value, names);
            collect_used_names(&right.value, names);
        }
        Expr::IfElse {
            branches,
            else_branch,
        } => {
            for branch in branches {
                collect_used_names(&branch.condition.value, names);
                collect_used_names(&branch.then_branch.value, names);
            }
            collect_used_names(&else_branch.value, names);
        }
        Expr::CaseOf {
            expr: subject,
            branches,
        } => {
            collect_used_names(&subject.value, names);
            for b in branches {
                collect_used_names(&b.body.value, names);
            }
        }
        Expr::LetIn {
            declarations, body, ..
        } => {
            for d in declarations {
                match &d.value {
                    elm_ast::expr::LetDeclaration::Function(f) => {
                        collect_used_names(&f.declaration.value.body.value, names);
                    }
                    elm_ast::expr::LetDeclaration::Destructuring { body: b, .. } => {
                        collect_used_names(&b.value, names);
                    }
                }
            }
            collect_used_names(&body.value, names);
        }
        Expr::Lambda { body, .. } => collect_used_names(&body.value, names),
        Expr::Parenthesized { expr: inner, .. } | Expr::Negation(inner) => {
            collect_used_names(&inner.value, names);
        }
        Expr::Tuple(elems) => {
            for e in elems {
                collect_used_names(&e.value, names);
            }
        }
        Expr::List {
            elements: elems, ..
        } => {
            for e in elems {
                collect_used_names(&e.value, names);
            }
        }
        Expr::Record(fields) => {
            for f in fields {
                collect_used_names(&f.value.value.value, names);
            }
        }
        Expr::RecordUpdate { updates, .. } => {
            for f in updates {
                collect_used_names(&f.value.value.value, names);
            }
        }
        Expr::RecordAccess { record, .. } => {
            collect_used_names(&record.value, names);
        }
        _ => {}
    }
}
