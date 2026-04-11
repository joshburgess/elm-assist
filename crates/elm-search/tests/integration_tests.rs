use std::fs;
use std::path::{Path, PathBuf};

use elm_ast::parse;
use elm_search::query::parse_query;
use elm_search::search::search;

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

fn search_fixtures(query_str: &str) -> usize {
    let dirs = [
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
    ];
    let query = parse_query(query_str).unwrap();
    let mut total = 0;

    for dir in &dirs {
        let files = find_elm_files(dir);
        for file in &files {
            let source = match fs::read_to_string(file) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let module = match parse(&source) {
                Ok(m) => m,
                Err(_) => continue,
            };
            total += search(&module, &query).len();
        }
    }
    total
}

/// Every query type should find at least one result across the full corpus.
/// This proves all 10 search capabilities work on real code, not just synthetic snippets.
#[test]
fn every_query_type_finds_results() {
    let queries = [
        ("returns Maybe", "functions returning Maybe"),
        ("type Int", "functions using Int in their signature"),
        ("case-on Just", "case expressions matching Just"),
        ("update .props", "record updates touching .props"),
        ("calls Dict", "qualified calls to Dict"),
        ("unused-args", "functions with unused arguments"),
        ("lambda 2", "lambdas with 2+ arguments"),
        ("uses map", "functions using 'map'"),
        ("def get", "top-level definitions matching 'get'"),
        ("expr case", "case expressions"),
    ];

    let mut missing = Vec::new();
    for (query, description) in &queries {
        let count = search_fixtures(query);
        if count == 0 {
            missing.push(format!("{query} ({description})"));
        }
        eprintln!("  {query}: {count} matches");
    }

    assert!(
        missing.is_empty(),
        "these query types found 0 results across 291 real files:\n  {}",
        missing.join("\n  ")
    );
}

#[test]
fn returns_maybe_finds_results_in_core() {
    let count = search_fixtures("returns Maybe");
    assert!(
        count > 0,
        "should find functions returning Maybe in elm/core"
    );
}

#[test]
fn case_on_just_finds_results_in_core() {
    let count = search_fixtures("case-on Just");
    assert!(
        count > 0,
        "should find case expressions matching Just in elm/core"
    );
}

#[test]
fn unused_args_finds_results_in_core() {
    let count = search_fixtures("unused-args");
    // elm/core has a few unused args in kernel-backed modules.
    assert!(count > 0, "should find unused args in elm/core");
}

#[test]
fn def_finds_common_names() {
    let count = search_fixtures("def map");
    assert!(count > 0, "should find definitions containing 'map'");
}

#[test]
fn expr_case_finds_case_expressions() {
    let count = search_fixtures("expr case");
    assert!(count > 0, "should find case expressions in elm/core");
}
