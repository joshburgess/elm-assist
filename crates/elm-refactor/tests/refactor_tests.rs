use elm_ast::{parse, print};
use elm_refactor::commands;
use test_better::prelude::*;

/// Parse source, apply a transformation to the module, print back.
fn _transform(source: &str, f: impl FnOnce(&mut elm_ast::file::ElmModule)) -> String {
    let mut module = parse(source).unwrap_or_else(|e| panic!("parse failed: {e:?}"));
    f(&mut module);
    print(&module)
}

// ── Sort imports ─────────────────────────────────────────────────────

#[test]
fn sort_imports_alphabetizes() -> TestResult {
    let source = "\
module Main exposing (..)

import Html
import Dict
import Array
import Basics

x = 1
";
    let mut project = make_project(source);
    let changes = commands::sort_imports::sort_imports(&mut project);
    check!(changes).satisfies(eq(1))?;

    let printed = print(&project.files[0].module);
    let mut import_names: Vec<&str> = Vec::new();
    for l in printed.lines().filter(|l| l.starts_with("import ")) {
        let name = l
            .trim_start_matches("import ")
            .split_whitespace()
            .next()
            .or_fail_with("import name")?;
        import_names.push(name);
    }
    check!(import_names).satisfies(eq(vec!["Array", "Basics", "Dict", "Html"]))?;
    Ok(())
}

#[test]
fn sort_imports_already_sorted() -> TestResult {
    let source = "\
module Main exposing (..)

import Array
import Dict
import Html

x = 1
";
    let mut project = make_project(source);
    let changes = commands::sort_imports::sort_imports(&mut project);
    check!(changes).satisfies(eq(0))?;
    Ok(())
}

// ── Rename ───────────────────────────────────────────────────────────

#[test]
fn rename_function_in_defining_module() -> TestResult {
    let source = "\
module Main exposing (old)

old x = x + 1

y = old 5
";
    let mut project = make_project(source);
    let changes = commands::rename::rename(&mut project, "Main", "old", "new");
    check!(changes > 0).satisfies(is_true())?;

    let printed = print(&project.files[0].module);
    check!(printed.as_str()).satisfies(contains_str("new x ="))?;
    check!(printed.as_str()).satisfies(contains_str("y =\n    new 5"))?;
    check!(printed.contains("old")).satisfies(is_false())?;
    Ok(())
}

#[test]
fn rename_updates_exposing_list() -> TestResult {
    let source = "\
module Main exposing (myFunc)

myFunc x = x
";
    let mut project = make_project(source);
    commands::rename::rename(&mut project, "Main", "myFunc", "renamed");

    let printed = print(&project.files[0].module);
    check!(printed.as_str()).satisfies(contains_str("exposing (renamed)"))?;
    check!(printed.contains("myFunc")).satisfies(is_false())?;
    Ok(())
}

// ── Qualify imports ──────────────────────────────────────────────────

#[test]
fn qualify_imports_converts_exposed_to_qualified() -> TestResult {
    let source = "\
module Main exposing (..)

import List exposing (map, filter)

x = map f (filter g list)
";
    let mut project = make_project(source);
    let changes = commands::qualify_imports::qualify_imports(&mut project);
    check!(changes > 0).satisfies(is_true())?;

    let printed = print(&project.files[0].module);
    check!(printed.as_str()).satisfies(contains_str("List.map"))?;
    check!(printed.as_str()).satisfies(contains_str("List.filter"))?;
    // The exposing list should be removed or empty.
    check!(printed.contains("exposing (map")).satisfies(is_false())?;
    Ok(())
}

#[test]
fn qualify_imports_no_change_when_already_qualified() -> TestResult {
    let source = "\
module Main exposing (..)

import List

x = List.map f list
";
    let mut project = make_project(source);
    let changes = commands::qualify_imports::qualify_imports(&mut project);
    check!(changes).satisfies(eq(0))?;
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────

fn make_project(source: &str) -> elm_refactor::project::Project {
    let module = parse(source).unwrap_or_else(|e| panic!("parse failed: {e:?}"));
    let module_name = match &module.header.value {
        elm_ast::module_header::ModuleHeader::Normal { name, .. }
        | elm_ast::module_header::ModuleHeader::Port { name, .. }
        | elm_ast::module_header::ModuleHeader::Effect { name, .. } => name.value.join("."),
    };
    elm_refactor::project::Project {
        files: vec![elm_refactor::project::ProjectFile {
            path: std::path::PathBuf::from("test.elm"),
            source: source.to_string(),
            module,
            module_name,
        }],
    }
}
