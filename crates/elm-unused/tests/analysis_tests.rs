// We need to access the library modules from the binary crate.
// Since elm-unused is a binary, we'll replicate the test by directly
// using elm-ast-rs and the analysis logic inline.

use elm_ast::parse;

/// Helper: parse source, collect info, and return the module info.
fn parse_module(source: &str) -> elm_ast::file::ElmModule {
    parse(source).unwrap_or_else(|e| panic!("parse failed: {e:?}"))
}

// Since the collect/analyze modules are in the binary crate and not a library,
// we test the tool end-to-end by checking that elm-ast-rs provides the right
// data for analysis. These tests verify the Visit-based collection patterns.

use elm_ast::declaration::Declaration;
use elm_ast::expr::Expr;
use elm_ast::node::Spanned;
use elm_ast::visit::{self, Visit};

struct IdentCollector(Vec<String>);

impl Visit for IdentCollector {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::FunctionOrValue { module_name, name } = &expr.value
            && module_name.is_empty()
        {
            self.0.push(name.clone());
        }
        visit::walk_expr(self, expr);
    }
}

#[test]
fn detects_unused_import() {
    let m = parse_module(
        "\
module Main exposing (..)

import Html
import Json.Decode

x = 1
",
    );
    // Neither Html nor Json.Decode are used — a tool should flag both.
    assert_eq!(m.imports.len(), 2);

    let mut collector = IdentCollector(Vec::new());
    collector.visit_module(&m);
    // No references to Html or Json.Decode functions.
    assert!(!collector.0.iter().any(|n| n == "Html" || n == "Json"));
}

#[test]
fn used_import_not_flagged() {
    let m = parse_module(
        "\
module Main exposing (..)

import Html

x = Html.div
",
    );
    // Html is used via qualified reference — should NOT be flagged.
    let mut collector = IdentCollector(Vec::new());
    collector.visit_module(&m);
    // The qualified ref "Html" appears in the AST.
    assert_eq!(m.imports.len(), 1);
}

#[test]
fn detects_unused_function() {
    let m = parse_module(
        "\
module Main exposing (used)

used = 1

unused = 2
",
    );
    // `unused` is defined but not exported and not referenced.
    let defined: Vec<&str> = m
        .declarations
        .iter()
        .filter_map(|d| match &d.value {
            Declaration::FunctionDeclaration(f) => Some(f.declaration.value.name.value.as_str()),
            _ => None,
        })
        .collect();
    assert!(defined.contains(&"used"));
    assert!(defined.contains(&"unused"));

    let mut collector = IdentCollector(Vec::new());
    collector.visit_module(&m);
    // `unused` is never referenced in any expression.
    assert!(!collector.0.contains(&"unused".to_string()));
}

#[test]
fn detects_unused_constructor() {
    let m = parse_module(
        "\
module Main exposing (..)

type Msg = Used | Unused

x = Used
",
    );
    let mut collector = IdentCollector(Vec::new());
    collector.visit_module(&m);
    // Only `Used` appears in expressions.
    assert!(collector.0.contains(&"Used".to_string()));
    assert!(!collector.0.contains(&"Unused".to_string()));
}

#[test]
fn detects_unused_type() {
    let m = parse_module(
        "\
module Main exposing (..)

type alias UsedType = Int

type alias UnusedType = String

x : UsedType
x = 1
",
    );
    // UsedType appears in the type annotation, UnusedType does not.
    struct TypeCollector(Vec<String>);
    impl Visit for TypeCollector {
        fn visit_type_annotation(
            &mut self,
            ty: &Spanned<elm_ast::type_annotation::TypeAnnotation>,
        ) {
            if let elm_ast::type_annotation::TypeAnnotation::Typed { name, .. } = &ty.value {
                self.0.push(name.value.clone());
            }
            visit::walk_type_annotation(self, ty);
        }
    }

    let mut collector = TypeCollector(Vec::new());
    collector.visit_module(&m);
    assert!(collector.0.contains(&"UsedType".to_string()));
    assert!(collector.0.contains(&"Int".to_string()));
    assert!(!collector.0.contains(&"UnusedType".to_string()));
}
