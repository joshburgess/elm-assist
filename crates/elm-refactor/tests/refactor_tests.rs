use elm_ast::{parse, print};
use elm_refactor::commands;

/// Parse source, apply a transformation to the module, print back.
fn _transform(source: &str, f: impl FnOnce(&mut elm_ast::file::ElmModule)) -> String {
    let mut module = parse(source).unwrap_or_else(|e| panic!("parse failed: {e:?}"));
    f(&mut module);
    print(&module)
}

// ── Sort imports ─────────────────────────────────────────────────────

#[test]
fn sort_imports_alphabetizes() {
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
    assert_eq!(changes, 1);

    let printed = print(&project.files[0].module);
    let import_names: Vec<&str> = printed
        .lines()
        .filter(|l| l.starts_with("import "))
        .map(|l| {
            l.trim_start_matches("import ")
                .split_whitespace()
                .next()
                .unwrap()
        })
        .collect();
    assert_eq!(import_names, vec!["Array", "Basics", "Dict", "Html"]);
}

#[test]
fn sort_imports_already_sorted() {
    let source = "\
module Main exposing (..)

import Array
import Dict
import Html

x = 1
";
    let mut project = make_project(source);
    let changes = commands::sort_imports::sort_imports(&mut project);
    assert_eq!(changes, 0);
}

// ── Rename ───────────────────────────────────────────────────────────

#[test]
fn rename_function_in_defining_module() {
    let source = "\
module Main exposing (old)

old x = x + 1

y = old 5
";
    let mut project = make_project(source);
    let changes = commands::rename::rename(&mut project, "Main", "old", "new");
    assert!(changes > 0);

    let printed = print(&project.files[0].module);
    assert!(printed.contains("new x ="));
    assert!(printed.contains("y =\n    new 5"));
    assert!(!printed.contains("old"));
}

#[test]
fn rename_updates_exposing_list() {
    let source = "\
module Main exposing (myFunc)

myFunc x = x
";
    let mut project = make_project(source);
    commands::rename::rename(&mut project, "Main", "myFunc", "renamed");

    let printed = print(&project.files[0].module);
    assert!(printed.contains("exposing (renamed)"));
    assert!(!printed.contains("myFunc"));
}

// ── Qualify imports ──────────────────────────────────────────────────

#[test]
fn qualify_imports_converts_exposed_to_qualified() {
    let source = "\
module Main exposing (..)

import List exposing (map, filter)

x = map f (filter g list)
";
    let mut project = make_project(source);
    let changes = commands::qualify_imports::qualify_imports(&mut project);
    assert!(changes > 0);

    let printed = print(&project.files[0].module);
    assert!(printed.contains("List.map"));
    assert!(printed.contains("List.filter"));
    // The exposing list should be removed or empty.
    assert!(!printed.contains("exposing (map"));
}

#[test]
fn qualify_imports_no_change_when_already_qualified() {
    let source = "\
module Main exposing (..)

import List

x = List.map f list
";
    let mut project = make_project(source);
    let changes = commands::qualify_imports::qualify_imports(&mut project);
    assert_eq!(changes, 0);
}

// ── Helpers ──────────────────────────────────────────────────────────

fn make_project(source: &str) -> elm_refactor::project::Project {
    let module = parse(source).unwrap();
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
