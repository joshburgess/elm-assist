use std::fs;
use std::path::{Path, PathBuf};

use elm_ast::module_header::ModuleHeader;
use elm_ast::parse;
use elm_refactor::commands::sort_imports::sort_imports;
use elm_refactor::project::{Project, ProjectFile};

fn find_elm_files(dir: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if Path::new(dir).exists() {
        collect_elm_files(&PathBuf::from(dir), &mut files);
        files.sort();
    }
    files
}

fn collect_elm_files(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_elm_files(&path, files);
            } else if path.extension().is_some_and(|ext| ext == "elm") {
                files.push(path);
            }
        }
    }
}

fn all_fixture_dirs() -> Vec<&'static str> {
    vec![
        "../../test-fixtures/core/src",
        "../../test-fixtures/html/src",
        "../../test-fixtures/browser/src",
        "../../test-fixtures/json/src",
        "../../test-fixtures/http/src",
        "../../test-fixtures/url/src",
        "../../test-fixtures/parser/src",
        "../../test-fixtures/virtual-dom/src",
        "../../test-fixtures/bytes/src",
        "../../test-fixtures/file/src",
        "../../test-fixtures/time/src",
        "../../test-fixtures/regex/src",
        "../../test-fixtures/random/src",
        "../../test-fixtures/svg/src",
        "../../test-fixtures/compiler/reactor/src",
        "../../test-fixtures/project-metadata-utils/src",
        "../../test-fixtures/test/src",
        "../../test-fixtures/markdown/src",
        "../../test-fixtures/linear-algebra/src",
        "../../test-fixtures/webgl/src",
        "../../test-fixtures/benchmark/src",
        "../../test-fixtures/list-extra/src",
        "../../test-fixtures/maybe-extra/src",
        "../../test-fixtures/string-extra/src",
        "../../test-fixtures/dict-extra/src",
        "../../test-fixtures/array-extra/src",
        "../../test-fixtures/result-extra/src",
        "../../test-fixtures/html-extra/src",
        "../../test-fixtures/json-extra/src",
        "../../test-fixtures/typed-svg/src",
        "../../test-fixtures/elm-json-decode-pipeline/src",
        "../../test-fixtures/elm-sweet-poll/src",
        "../../test-fixtures/elm-compare/src",
        "../../test-fixtures/elm-string-conversions/src",
        "../../test-fixtures/elm-sortable-table/src",
        "../../test-fixtures/elm-css/src",
        "../../test-fixtures/elm-hex/src",
        "../../test-fixtures/elm-iso8601-date-strings/src",
        "../../test-fixtures/elm-ui/src",
        "../../test-fixtures/elm-animator/src",
        "../../test-fixtures/elm-markdown/src",
        "../../test-fixtures/remotedata/src",
        "../../test-fixtures/murmur3/src",
        "../../test-fixtures/elm-round/src",
        "../../test-fixtures/elm-base64/src",
        "../../test-fixtures/elm-flate/src",
        "../../test-fixtures/elm-csv/src",
        "../../test-fixtures/elm-rosetree/src",
        "../../test-fixtures/assoc-list/src",
        "../../test-fixtures/elm-bool-extra/src",
    ]
}

fn load_all_fixtures() -> Project {
    let mut files = Vec::new();

    for dir in all_fixture_dirs() {
        for path in find_elm_files(dir) {
            let source = match fs::read_to_string(&path) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let module = match parse(&source) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let module_name = match &module.header.value {
                ModuleHeader::Normal { name, .. }
                | ModuleHeader::Port { name, .. }
                | ModuleHeader::Effect { name, .. } => name.value.join("."),
            };
            files.push(ProjectFile {
                path,
                source,
                module,
                module_name,
            });
        }
    }

    Project { files }
}

/// sort_imports should not panic on any real-world project.
#[test]
fn sort_imports_no_crash_on_all_fixtures() {
    let mut project = load_all_fixtures();
    assert!(!project.files.is_empty(), "no fixture files loaded");

    // Should not panic.
    let changes = sort_imports(&mut project);

    eprintln!(
        "sort_imports ran on {} files, changed {}",
        project.files.len(),
        changes
    );
}

/// After sort_imports, every module's imports should actually be sorted.
#[test]
fn sort_imports_produces_sorted_output() {
    let mut project = load_all_fixtures();
    sort_imports(&mut project);

    for file in &project.files {
        let import_names: Vec<String> = file
            .module
            .imports
            .iter()
            .map(|i| i.value.module_name.value.join("."))
            .collect();

        let mut sorted = import_names.clone();
        sorted.sort();

        assert_eq!(
            import_names, sorted,
            "imports should be sorted in {}: got {:?}",
            file.module_name, import_names
        );
    }
}

/// sort_imports result should still be parseable (round-trips).
#[test]
fn sort_imports_output_reparses() {
    let mut project = load_all_fixtures();
    sort_imports(&mut project);

    let mut failures = Vec::new();
    for file in &project.files {
        let printed = elm_ast::print(&file.module);
        if let Err(errors) = parse(&printed) {
            failures.push(format!(
                "{}: {}",
                file.module_name,
                errors
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "sort_imports output failed to reparse:\n{}",
        failures.join("\n")
    );
}

/// qualify_imports should not panic and should find work to do.
#[test]
fn qualify_imports_no_crash_on_all_fixtures() {
    use elm_refactor::commands::qualify_imports::qualify_imports;

    let mut project = load_all_fixtures();
    assert!(!project.files.is_empty(), "no fixture files loaded");

    let changes = qualify_imports(&mut project);

    // Real packages use `exposing (foo)` patterns, so there should be work.
    assert!(
        changes > 0,
        "qualify_imports should make changes on real code"
    );

    eprintln!(
        "qualify_imports ran on {} files, made {} changes",
        project.files.len(),
        changes
    );
}

/// qualify_imports result should still be parseable.
#[test]
fn qualify_imports_output_reparses() {
    use elm_refactor::commands::qualify_imports::qualify_imports;

    let mut project = load_all_fixtures();
    qualify_imports(&mut project);

    let mut failures = Vec::new();
    for file in &project.files {
        let printed = elm_ast::print(&file.module);
        if let Err(errors) = parse(&printed) {
            failures.push(format!(
                "{}: {}",
                file.module_name,
                errors
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "qualify_imports output failed to reparse:\n{}",
        failures.join("\n")
    );
}
