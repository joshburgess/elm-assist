use elm_ast::{parse, print};
use elm_refactor::commands;
use elm_refactor::project::{Project, ProjectFile};
use std::path::PathBuf;
use test_better::prelude::*;

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
        .unwrap_or_else(|| panic!("module {module_name} not found"));
    print(&file.module)
}

// ── Cross-file rename ────────────────────────────────────────────────

#[test]
fn rename_updates_definition_and_cross_file_reference() -> TestResult {
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
    check!(changes > 0).satisfies(is_true())?;

    // Definition renamed.
    let utils = printed(&project, "Utils");
    check!(utils.as_str()).satisfies(contains_str("assist x ="))?;
    check!(utils.as_str()).satisfies(contains_str("exposing (assist)"))?;
    check!(utils.contains("helper")).satisfies(is_false())?;

    // Qualified reference renamed.
    let main = printed(&project, "Main");
    check!(main.as_str()).satisfies(contains_str("Utils.assist"))?;
    check!(main.contains("Utils.helper")).satisfies(is_false())?;
    Ok(())
}

#[test]
fn rename_updates_exposed_import() -> TestResult {
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
    check!(main.as_str()).satisfies(contains_str("exposing (new)"))?;
    check!(main.as_str()).satisfies(contains_str("new 5"))?;
    check!(main.contains("old")).satisfies(is_false())?;
    Ok(())
}

#[test]
fn rename_no_false_positives() -> TestResult {
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
    check!(b.as_str()).satisfies(contains_str("A.baz"))?;
    check!(b.as_str()).satisfies(contains_str("foo =\n    99"))?; // B.foo unchanged
    Ok(())
}

// ── Sort imports across files ────────────────────────────────────────

#[test]
fn sort_imports_works_across_files() -> TestResult {
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
    check!(changes).satisfies(eq(2))?;

    let a = printed(&project, "A");
    let a_imports: Vec<&str> = a.lines().filter(|l| l.starts_with("import ")).collect();
    check!(a_imports).satisfies(eq(vec!["import A", "import M", "import Z"]))?;

    let b = printed(&project, "B");
    let b_imports: Vec<&str> = b.lines().filter(|l| l.starts_with("import ")).collect();
    check!(b_imports).satisfies(eq(vec!["import B", "import D", "import X"]))?;
    Ok(())
}
