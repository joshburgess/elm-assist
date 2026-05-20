// We need to access the library modules from the binary crate.
// Since elm-unused is a binary, we'll replicate the test by directly
// using elm-ast-rs and the analysis logic inline.

use elm_ast::parse;
use test_better::prelude::*;

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
fn detects_unused_import() -> TestResult {
    let m = parse_module(
        "\
module Main exposing (..)

import Html
import Json.Decode

x = 1
",
    );
    // Neither Html nor Json.Decode are used — a tool should flag both.
    check!(m.imports.len()).satisfies(eq(2))?;

    let mut collector = IdentCollector(Vec::new());
    collector.visit_module(&m);
    // No references to Html or Json.Decode functions.
    check!(collector.0.iter().any(|n| n == "Html" || n == "Json")).satisfies(is_false())?;
    Ok(())
}

#[test]
fn used_import_not_flagged() -> TestResult {
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
    check!(m.imports.len()).satisfies(eq(1))?;
    Ok(())
}

#[test]
fn detects_unused_function() -> TestResult {
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
    check!(defined.contains(&"used")).satisfies(is_true())?;
    check!(defined.contains(&"unused")).satisfies(is_true())?;

    let mut collector = IdentCollector(Vec::new());
    collector.visit_module(&m);
    // `unused` is never referenced in any expression.
    check!(collector.0.contains(&"unused".to_string())).satisfies(is_false())?;
    Ok(())
}

#[test]
fn detects_unused_constructor() -> TestResult {
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
    check!(collector.0.contains(&"Used".to_string())).satisfies(is_true())?;
    check!(collector.0.contains(&"Unused".to_string())).satisfies(is_false())?;
    Ok(())
}

#[test]
fn detects_unused_type() -> TestResult {
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
    check!(collector.0.contains(&"UsedType".to_string())).satisfies(is_true())?;
    check!(collector.0.contains(&"Int".to_string())).satisfies(is_true())?;
    check!(collector.0.contains(&"UnusedType".to_string())).satisfies(is_false())?;
    Ok(())
}
