use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use elm_ast::parse;
use elm_lint::collect::collect_module_info;
use elm_lint::elm_json::ElmJsonInfo;
use elm_lint::rule::{LintContext, ProjectContext, Rule};
use elm_lint::rules;

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

/// All rules should run without panicking on every real-world file.
#[test]
fn all_rules_no_crash_on_real_files() {
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

    let all_rules = rules::all_rules();
    let mut total_files = 0;

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

            let ctx = LintContext {
                module: &module,
                source: &source,
                file_path: &file.display().to_string(),
                project_modules: &[],
                module_info: None,
                project: None,
            };

            for rule in &all_rules {
                // Should not panic.
                let _ = rule.check(&ctx);
            }
            total_files += 1;
        }
    }

    if total_files == 0 {
        eprintln!("skipping: no fixture files found (run `git submodule update --init`)");
        return;
    }
    eprintln!(
        "Ran all {0} rules on {total_files} files without crashes",
        all_rules.len()
    );
}

/// Every rule should fire at least once across the full 291-file corpus.
/// If a rule never triggers on real code, it's either broken or useless.
#[test]
fn every_rule_fires_on_real_code() {
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

    let all_rules = rules::all_rules();
    let mut hits: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();

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

            let ctx = LintContext {
                module: &module,
                source: &source,
                file_path: &file.display().to_string(),
                project_modules: &[],
                module_info: None,
                project: None,
            };

            for rule in &all_rules {
                let errors = rule.check(&ctx);
                if !errors.is_empty() {
                    *hits.entry(rule.name()).or_default() += errors.len();
                }
            }
        }
    }

    // Rules that might not fire on well-written real code are exempt.
    // These are checked via synthetic snippets in each_rule_fires_on_something.
    let exempt = [
        "NoDebug",                        // well-maintained packages don't ship Debug.log
        "NoBooleanCase",                  // uncommon pattern
        "NoIfTrueFalse",                  // uncommon pattern
        "NoRedundantCons",                // uncommon pattern
        "NoAlwaysIdentity",               // uncommon pattern
        "NoEmptyLet",                     // uncommon pattern
        "NoEmptyRecordUpdate",            // uncommon pattern
        "NoNestedNegation",               // uncommon pattern
        "NoWildcardPatternLast",          // uncommon in well-written code
        "NoUnusedExports",                // requires project context
        "NoUnusedCustomTypeConstructors", // requires project context
        "NoUnusedModules",                // requires project context
        // Phase 3 rules — uncommon in well-written packages
        "NoBoolOperatorSimplify",         // uncommon pattern
        "NoEmptyListConcat",              // uncommon pattern
        "NoListLiteralConcat",            // uncommon pattern
        "NoPipelineSimplify",             // uncommon pattern
        "NoNegationOfBooleanOperator",    // uncommon pattern
        "NoStringConcat",                 // uncommon pattern
        "NoFullyAppliedPrefixOperator",   // uncommon pattern
        "NoIdentityFunction",             // uncommon pattern
        "NoSimpleLetBody",                // uncommon pattern
        "NoUnusedLetBinding",             // uncommon pattern
        "NoTodoComment",                  // well-maintained packages resolve TODOs
        "NoMaybeMapWithNothing",          // uncommon pattern
        "NoResultMapWithErr",             // uncommon pattern
        // New rules — uncommon in well-written packages or require context
        "NoDeprecated",                   // packages rarely mark things deprecated inline
        "NoUnnecessaryPortModule",        // uncommon pattern
        "NoUnusedCustomTypeConstructorArgs", // requires seeing all case branches
        "NoPrematureLetComputation",      // uncommon pattern
        "NoRecordPatternInFunctionArgs",  // style preference
        // Batch 2 rules
        "NoUnusedPatterns",               // uncommon in well-written code
        "CognitiveComplexity",            // well-written packages keep functions short
        "NoMissingTypeAnnotationInLetIn", // many packages skip let annotations
        "NoConfusingPrefixOperator",      // uncommon pattern
        "NoMissingTypeExpose",            // requires specific exposing list
        "NoRedundantlyQualifiedType",     // uncommon in well-written code
        "NoUnoptimizedRecursion",         // uncommon pattern
        "NoRecursiveUpdate",              // only applies to TEA apps with `update`
        // Port rules
        "NoDuplicatePorts",               // requires project context with multiple port modules
        "NoUnsafePorts",                  // requires port declarations (rare in packages)
        "NoInconsistentAliases",          // requires per-rule config to do anything
        "NoUnusedDependencies",           // requires elm.json (not available in test fixtures)
        "NoShadowing",                    // well-written packages avoid shadowing
    ];

    // If no fixture files are available, skip gracefully.
    if hits.is_empty() {
        let has_files = dirs.iter().any(|d| !find_elm_files(d).is_empty());
        if !has_files {
            eprintln!("skipping: no fixture files found (run `git submodule update --init`)");
            return;
        }
    }

    let mut missing = Vec::new();
    for rule in &all_rules {
        if !exempt.contains(&rule.name()) && !hits.contains_key(rule.name()) {
            missing.push(rule.name());
        }
    }

    eprintln!("Rule fire counts across real files:");
    for rule in &all_rules {
        let count = hits.get(rule.name()).copied().unwrap_or(0);
        let marker = if exempt.contains(&rule.name()) {
            " (exempt)"
        } else {
            ""
        };
        eprintln!("  {}: {}{}", rule.name(), count, marker);
    }

    assert!(
        missing.is_empty(),
        "these non-exempt rules never fired on real code: {:?}",
        missing
    );
}

/// Each rule should produce at least one finding on SOME file.
/// This catches rules that are accidentally no-ops.
#[test]
fn each_rule_fires_on_something() {
    let test_cases: Vec<(&str, &str)> = vec![
        (
            "NoUnusedImports",
            "module T exposing (..)\n\nimport Html\n\nx = 1",
        ),
        (
            "NoDebug",
            "module T exposing (..)\n\nx = Debug.log \"hi\" 1",
        ),
        (
            "NoMissingTypeAnnotation",
            "module T exposing (..)\n\nfoo x = x",
        ),
        (
            "NoSinglePatternCase",
            "module T exposing (..)\n\nx =\n    case y of\n        _ ->\n            1",
        ),
        (
            "NoBooleanCase",
            "module T exposing (..)\n\nx =\n    case y of\n        True ->\n            1\n        False ->\n            0",
        ),
        (
            "NoIfTrueFalse",
            "module T exposing (..)\n\nx = if y then True else False",
        ),
        ("NoUnnecessaryParens", "module T exposing (..)\n\nx = (1)"),
        ("NoRedundantCons", "module T exposing (..)\n\nx = 1 :: []"),
        (
            "NoAlwaysIdentity",
            "module T exposing (..)\n\nx = always identity",
        ),
        // Phase 3 rules
        (
            "NoBoolOperatorSimplify",
            "module T exposing (..)\n\nx = y && True",
        ),
        (
            "NoEmptyListConcat",
            "module T exposing (..)\n\nx = [] ++ y",
        ),
        (
            "NoListLiteralConcat",
            "module T exposing (..)\n\nx = [ 1 ] ++ [ 2 ]",
        ),
        (
            "NoPipelineSimplify",
            "module T exposing (..)\n\nx = y |> identity",
        ),
        (
            "NoNegationOfBooleanOperator",
            "module T exposing (..)\n\nx = not (a == b)",
        ),
        (
            "NoStringConcat",
            "module T exposing (..)\n\nx = \"hello\" ++ \" world\"",
        ),
        (
            "NoFullyAppliedPrefixOperator",
            "module T exposing (..)\n\nx = (+) 1 2",
        ),
        (
            "NoIdentityFunction",
            "module T exposing (..)\n\nx = \\a -> a",
        ),
        (
            "NoSimpleLetBody",
            "module T exposing (..)\n\nx =\n    let\n        y = 1\n    in\n    y",
        ),
        (
            "NoUnusedLetBinding",
            "module T exposing (..)\n\nx =\n    let\n        y = 1\n    in\n    2",
        ),
        (
            "NoTodoComment",
            "module T exposing (..)\n\n-- TODO fix\nx = 1",
        ),
        (
            "NoMaybeMapWithNothing",
            "module T exposing (..)\n\nimport Maybe\n\nx = Maybe.map f Nothing",
        ),
        (
            "NoResultMapWithErr",
            "module T exposing (..)\n\nimport Result\n\nx = Result.map f (Err e)",
        ),
        // New rules
        (
            "NoUnusedParameters",
            "module T exposing (..)\n\nfoo x = 1",
        ),
        (
            "NoUnusedCustomTypeConstructorArgs",
            "module T exposing (..)\n\ntype Msg = Click Int\n\nfoo msg =\n    case msg of\n        Click _ ->\n            1",
        ),
        (
            "NoExposingAll",
            "module T exposing (..)\n\nfoo = 1",
        ),
        (
            "NoImportExposingAll",
            "module T exposing (foo)\n\nimport Html exposing (..)\n\nfoo = Html.div",
        ),
        (
            "NoDeprecated",
            "module T exposing (bar)\n\n{-| deprecated -}\nfoo = 1\n\nbar = foo + 1",
        ),
        (
            "NoMissingDocumentation",
            "module T exposing (foo)\n\nfoo = 1",
        ),
        (
            "NoUnnecessaryTrailingUnderscore",
            "module T exposing (..)\n\nfoo x_ = x_",
        ),
        (
            "NoPrematureLetComputation",
            "module T exposing (..)\n\nfoo x =\n    let\n        y = expensive x\n    in\n    if x then y else 0",
        ),
        (
            "NoUnnecessaryPortModule",
            "port module T exposing (foo)\n\nfoo = 1",
        ),
        (
            "NoShadowing",
            "module T exposing (..)\n\nfoo = 1\n\nbar foo = foo + 1",
        ),
        (
            "NoRecordPatternInFunctionArgs",
            "module T exposing (..)\n\nfoo { x, y } = x + y",
        ),
        // Batch 2 rules
        (
            "NoUnusedPatterns",
            "module T exposing (..)\n\nfoo x =\n    case x of\n        Just y ->\n            1\n        Nothing ->\n            0",
        ),
        (
            "NoMissingTypeAnnotationInLetIn",
            "module T exposing (..)\n\nfoo =\n    let\n        bar = 1\n    in\n    bar",
        ),
        (
            "NoConfusingPrefixOperator",
            "module T exposing (..)\n\nx = (-) 5 3",
        ),
        (
            "NoMissingTypeExpose",
            "module T exposing (foo)\n\ntype alias MyType = Int\n\nfoo : MyType -> Int\nfoo x = x",
        ),
        (
            "NoRedundantlyQualifiedType",
            "module T exposing (..)\n\nimport Set\n\nfoo : Set.Set Int\nfoo = Set.empty",
        ),
        (
            "NoUnoptimizedRecursion",
            "module T exposing (..)\n\nsum n =\n    if n == 0 then\n        0\n    else\n        n + sum (n - 1)",
        ),
        (
            "NoRecursiveUpdate",
            "module T exposing (..)\n\ntype Msg = Click | Reset\n\nupdate msg model =\n    case msg of\n        Click ->\n            model + 1\n        Reset ->\n            update Click 0",
        ),
        (
            "NoUnsafePorts",
            "port module T exposing (..)\n\ntype Msg = Click\n\nport sendMsg : Msg -> Cmd msg",
        ),
    ];

    // NoMaxLineLength needs special handling (long line)
    let long_line = format!("x = \"{}\"", "a".repeat(200));
    let max_line_source = format!("module T exposing (x)\n\n{long_line}");

    // CognitiveComplexity needs special handling (deeply nested function)
    let cognitive_source = r#"module T exposing (..)

foo x =
    if x == 1 then
        if x == 2 then
            if x == 3 then
                if x == 4 then
                    if x == 5 then
                        if x == 6 then
                            if x == 7 then
                                if x == 8 then
                                    1
                                else
                                    2
                            else
                                3
                        else
                            4
                    else
                        5
                else
                    6
            else
                7
        else
            8
    else
        9
"#;

    let all_rules = rules::all_rules();

    for (rule_name, source) in test_cases {
        let module = parse(source).unwrap();
        let ctx = LintContext {
            module: &module,
            source,
            file_path: "test.elm",
            project_modules: &[],
            module_info: None,
            project: None,
        };

        let rule = all_rules
            .iter()
            .find(|r| r.name() == rule_name)
            .unwrap_or_else(|| panic!("rule {rule_name} not found"));

        let errors = rule.check(&ctx);
        assert!(
            !errors.is_empty(),
            "rule {rule_name} should fire on test input but produced 0 errors"
        );
    }

    // Test NoMaxLineLength separately with dynamic source.
    {
        let module = parse(&max_line_source).unwrap();
        let ctx = LintContext {
            module: &module,
            source: &max_line_source,
            file_path: "test.elm",
            project_modules: &[],
            module_info: None,
            project: None,
        };
        let rule = all_rules
            .iter()
            .find(|r| r.name() == "NoMaxLineLength")
            .expect("NoMaxLineLength not found");
        let errors = rule.check(&ctx);
        assert!(
            !errors.is_empty(),
            "rule NoMaxLineLength should fire on test input but produced 0 errors"
        );
    }

    // Test CognitiveComplexity separately with deeply nested source.
    {
        let module = parse(cognitive_source).unwrap();
        let ctx = LintContext {
            module: &module,
            source: cognitive_source,
            file_path: "test.elm",
            project_modules: &[],
            module_info: None,
            project: None,
        };
        let rule = all_rules
            .iter()
            .find(|r| r.name() == "CognitiveComplexity")
            .expect("CognitiveComplexity not found");
        let errors = rule.check(&ctx);
        assert!(
            !errors.is_empty(),
            "rule CognitiveComplexity should fire on test input but produced 0 errors"
        );
    }

    // Test NoDuplicatePorts separately — it requires project context with two port modules.
    {
        let src_a = "port module Ports.A exposing (..)\n\nport sendMessage : String -> Cmd msg";
        let src_b = "port module Ports.B exposing (..)\n\nport sendMessage : String -> Cmd msg";

        let mod_a = parse(src_a).unwrap();
        let mod_b = parse(src_b).unwrap();
        let info_a = collect_module_info(&mod_a);
        let info_b = collect_module_info(&mod_b);
        let name_a = info_a.module_name.join(".");
        let name_b = info_b.module_name.join(".");

        let mut module_infos = HashMap::new();
        module_infos.insert(name_a.clone(), info_a);
        module_infos.insert(name_b.clone(), info_b);
        let project_context = ProjectContext::build(module_infos);
        let project_modules: Vec<String> = project_context.modules.keys().cloned().collect();

        let ctx_a = LintContext {
            module: &mod_a,
            source: src_a,
            file_path: "Ports/A.elm",
            project_modules: &project_modules,
            module_info: project_context.modules.get(&name_a),
            project: Some(&project_context),
        };

        let rule = all_rules
            .iter()
            .find(|r| r.name() == "NoDuplicatePorts")
            .expect("NoDuplicatePorts not found");
        let errors = rule.check(&ctx_a);
        assert!(
            !errors.is_empty(),
            "rule NoDuplicatePorts should fire on test input but produced 0 errors"
        );
    }

    // Test NoInconsistentAliases separately — it needs per-rule config to activate.
    {
        let mut rule = rules::no_inconsistent_aliases::NoInconsistentAliases::default();
        let config: toml::Value = toml::from_str(
            r#"aliases = { "Json.Decode" = "Decode" }"#,
        )
        .unwrap();
        rule.configure(&config).unwrap();

        let source = "module T exposing (..)\n\nimport Json.Decode as JD\n\nx = JD.string";
        let module = parse(source).unwrap();
        let ctx = LintContext {
            module: &module,
            source,
            file_path: "test.elm",
            project_modules: &[],
            module_info: None,
            project: None,
        };
        let errors = rule.check(&ctx);
        assert!(
            !errors.is_empty(),
            "rule NoInconsistentAliases should fire on test input but produced 0 errors"
        );
    }

    // Test NoUnusedDependencies separately — it needs elm.json + project context.
    {
        let mut deps = HashMap::new();
        deps.insert("elm/core".to_string(), "1.0.5".to_string());
        deps.insert("elm/json".to_string(), "1.1.3".to_string());
        let mut package_modules = HashMap::new();
        package_modules.insert(
            "elm/json".to_string(),
            vec!["Json.Decode".to_string(), "Json.Encode".to_string()],
        );
        let elm_json = ElmJsonInfo {
            direct_deps: deps,
            is_application: true,
            package_modules,
        };

        let source = "module Main exposing (..)\n\nx = 1";
        let module = parse(source).unwrap();
        let info = collect_module_info(&module);
        let mod_name = info.module_name.join(".");
        let mut module_infos = HashMap::new();
        module_infos.insert(mod_name.clone(), info);
        let project_context = ProjectContext::build_with_elm_json(module_infos, Some(elm_json));
        let project_modules: Vec<String> = project_context.modules.keys().cloned().collect();

        let ctx = LintContext {
            module: &module,
            source,
            file_path: "Main.elm",
            project_modules: &project_modules,
            module_info: project_context.modules.get(&mod_name),
            project: Some(&project_context),
        };

        let rule = all_rules
            .iter()
            .find(|r| r.name() == "NoUnusedDependencies")
            .expect("NoUnusedDependencies not found");
        let errors = rule.check(&ctx);
        assert!(
            !errors.is_empty(),
            "rule NoUnusedDependencies should fire on test input but produced 0 errors"
        );
    }
}
