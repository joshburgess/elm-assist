use elm_ast::{parse, print};
use elm_refactor::commands;
use elm_refactor::project::{Project, ProjectFile};
use std::path::PathBuf;

fn make_project(files: Vec<(&str, &str)>) -> Project {
    let project_files = files
        .into_iter()
        .map(|(name, source)| {
            let module = parse(source).unwrap_or_else(|e| panic!("parse {name} failed: {e:?}"));
            let module_name = match &module.header.value {
                elm_ast::module_header::ModuleHeader::Normal { name, .. }
                | elm_ast::module_header::ModuleHeader::Port { name, .. }
                | elm_ast::module_header::ModuleHeader::Effect { name, .. } => name.value.join("."),
            };
            ProjectFile {
                path: PathBuf::from(name),
                source: source.to_string(),
                module,
                module_name,
            }
        })
        .collect();
    Project {
        files: project_files,
    }
}

fn printed(project: &Project, module_name: &str) -> String {
    let file = project
        .files
        .iter()
        .find(|f| f.module_name == module_name)
        .unwrap();
    print(&file.module)
}

// ── Cross-file rename ────────────────────────────────────────────────

#[test]
fn rename_updates_definition_and_cross_file_reference() {
    let mut project = make_project(vec![
        (
            "Utils.elm",
            "\
module Utils exposing (helper)

helper x = x + 1
",
        ),
        (
            "Main.elm",
            "\
module Main exposing (..)

import Utils

main = Utils.helper 5
",
        ),
    ]);

    let changes = commands::rename::rename(&mut project, "Utils", "helper", "assist");
    assert!(changes > 0);

    // Definition renamed.
    let utils = printed(&project, "Utils");
    assert!(utils.contains("assist x ="));
    assert!(utils.contains("exposing (assist)"));
    assert!(!utils.contains("helper"));

    // Qualified reference renamed.
    let main = printed(&project, "Main");
    assert!(main.contains("Utils.assist"));
    assert!(!main.contains("Utils.helper"));
}

#[test]
fn rename_updates_exposed_import() {
    let mut project = make_project(vec![
        (
            "Utils.elm",
            "\
module Utils exposing (old)

old x = x
",
        ),
        (
            "Main.elm",
            "\
module Main exposing (..)

import Utils exposing (old)

main = old 5
",
        ),
    ]);

    commands::rename::rename(&mut project, "Utils", "old", "new");

    let main = printed(&project, "Main");
    assert!(main.contains("exposing (new)"));
    assert!(main.contains("new 5"));
    assert!(!main.contains("old"));
}

#[test]
fn rename_no_false_positives() {
    let mut project = make_project(vec![
        (
            "A.elm",
            "\
module A exposing (foo)

foo x = x
",
        ),
        (
            "B.elm",
            "\
module B exposing (..)

import A

bar = A.foo 1

foo = 99
",
        ),
    ]);

    commands::rename::rename(&mut project, "A", "foo", "baz");

    // B's own `foo` should NOT be renamed.
    let b = printed(&project, "B");
    assert!(b.contains("A.baz"));
    assert!(b.contains("foo =\n    99")); // B.foo unchanged
}

// ── Sort imports across files ────────────────────────────────────────

#[test]
fn sort_imports_works_across_files() {
    let mut project = make_project(vec![
        (
            "A.elm",
            "\
module A exposing (..)

import Z
import A
import M

x = 1
",
        ),
        (
            "B.elm",
            "\
module B exposing (..)

import X
import B
import D

y = 2
",
        ),
    ]);

    let changes = commands::sort_imports::sort_imports(&mut project);
    assert_eq!(changes, 2);

    let a = printed(&project, "A");
    let a_imports: Vec<&str> = a.lines().filter(|l| l.starts_with("import ")).collect();
    assert_eq!(a_imports, vec!["import A", "import M", "import Z"]);

    let b = printed(&project, "B");
    let b_imports: Vec<&str> = b.lines().filter(|l| l.starts_with("import ")).collect();
    assert_eq!(b_imports, vec!["import B", "import D", "import X"]);
}
