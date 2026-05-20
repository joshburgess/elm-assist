use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use elm_ast::parse;
use elm_unused::collect::collect_module_info;
use test_better::ErrorKind;
use test_better::prelude::*;

fn fail(msg: impl Into<String>) -> TestError {
    TestError::new(ErrorKind::Assertion).with_message(msg.into())
}

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

/// collect_module_info should not panic on any real-world file.
#[test]
fn collect_no_crash_on_all_fixtures() -> TestResult {
    let mut total = 0;

    for dir in all_fixture_dirs() {
        for file in find_elm_files(dir) {
            let source = match fs::read_to_string(&file) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let module = match parse(&source) {
                Ok(m) => m,
                Err(_) => continue,
            };

            // Should not panic.
            let _info = collect_module_info(&module);
            total += 1;
        }
    }

    check!(total > 0)
        .satisfies(is_true())
        .context("no fixture files found")?;
    eprintln!("collect_module_info succeeded on {total} files");
    Ok(())
}

/// Full cross-module analysis should not panic when run on all fixtures together.
#[test]
fn analyze_no_crash_on_all_fixtures() -> TestResult {
    let mut modules = HashMap::new();

    for dir in all_fixture_dirs() {
        for file in find_elm_files(dir) {
            let source = match fs::read_to_string(&file) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let module = match parse(&source) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let info = collect_module_info(&module);
            let mod_name = info.module_name.join(".");
            modules.insert(mod_name, info);
        }
    }

    check!(!modules.is_empty())
        .satisfies(is_true())
        .context("no modules collected")?;

    // Should not panic.
    let findings = elm_unused::analyze::analyze(&modules);

    eprintln!(
        "analyze ran on {} modules, produced {} findings",
        modules.len(),
        findings.len()
    );
    Ok(())
}

/// Analysis should find real unused code across packages.
/// Individual packages analyzed in isolation will have "unused imports" for
/// external dependencies they import (since those modules aren't in the
/// analysis set). This validates that findings are produced and categorized.
#[test]
fn analyze_finds_findings_per_package() -> TestResult {
    use elm_unused::analyze::FindingKind;

    // Analyze elm/core in isolation — should find findings.
    let mut modules = HashMap::new();
    for file in find_elm_files("../../test-fixtures/core/src") {
        let source = match fs::read_to_string(&file) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let module = match parse(&source) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let info = collect_module_info(&module);
        let mod_name = info.module_name.join(".");
        modules.insert(mod_name, info);
    }

    let findings = elm_unused::analyze::analyze(&modules);

    // elm/core should have some findings when analyzed in isolation.
    check!(!findings.is_empty())
        .satisfies(is_true())
        .context("elm/core should produce findings when analyzed in isolation")?;

    // Verify findings have all the expected fields populated.
    for f in &findings {
        check!(!f.module_name.is_empty())
            .satisfies(is_true())
            .context("finding should have a module name")?;
        check!(!f.name.is_empty())
            .satisfies(is_true())
            .context("finding should have a name")?;
    }

    // Verify multiple finding kinds are represented across all fixtures.
    let mut modules = HashMap::new();
    for dir in all_fixture_dirs() {
        for file in find_elm_files(dir) {
            let source = match fs::read_to_string(&file) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let module = match parse(&source) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let info = collect_module_info(&module);
            let mod_name = info.module_name.join(".");
            modules.insert(mod_name, info);
        }
    }

    let findings = elm_unused::analyze::analyze(&modules);

    // With 291 files from 50 packages, we should see at least unused imports
    // (since not all packages are present as dependencies).
    let has_unused_import = findings.iter().any(|f| f.kind == FindingKind::UnusedImport);
    check!(has_unused_import)
        .satisfies(is_true())
        .context("should find unused imports across the full corpus")?;

    // Count findings by kind label.
    let mut kind_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for f in &findings {
        *kind_counts.entry(f.kind.label()).or_default() += 1;
    }

    eprintln!("Finding kinds found: {}", kind_counts.len());
    for (kind, count) in &kind_counts {
        eprintln!("  {}: {}", kind, count);
    }
    Ok(())
}

/// collect_module_info should extract definitions and references correctly.
#[test]
fn collect_extracts_definitions_and_references() -> TestResult {
    // elm/core's List module should have well-known definitions.
    let source = fs::read_to_string("../../test-fixtures/core/src/List.elm")
        .or_fail_with("List.elm should exist")?;
    let module = parse(&source).map_err(|e| fail(format!("List.elm should parse: {e:?}")))?;
    let info = collect_module_info(&module);

    check!(info.module_name).satisfies(eq(vec!["List".to_string()]))?;

    // List module defines well-known functions.
    check!(info.defined_values.contains("map"))
        .satisfies(is_true())
        .context("List should define 'map'")?;
    check!(info.defined_values.contains("filter"))
        .satisfies(is_true())
        .context("List should define 'filter'")?;
    check!(info.defined_values.contains("foldl"))
        .satisfies(is_true())
        .context("List should define 'foldl'")?;

    // Should have imports.
    check!(!info.imports.is_empty())
        .satisfies(is_true())
        .context("List module should have imports")?;

    // Should have used values (references to other functions).
    check!(!info.used_values.is_empty())
        .satisfies(is_true())
        .context("List module should reference other values")?;

    Ok(())
}
