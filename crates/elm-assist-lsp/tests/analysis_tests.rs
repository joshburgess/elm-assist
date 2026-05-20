use std::collections::HashMap;
use std::path::PathBuf;

use tower_lsp::lsp_types::Url;

use elm_lint::config::Config;
use elm_lint::rules;

use elm_assist_lsp::analysis;
use elm_assist_lsp::state::ServerState;
use test_better::prelude::*;

fn make_state() -> ServerState {
    let all_rules = rules::all_rules();
    let rule_descriptions = all_rules
        .iter()
        .map(|r| {
            (
                r.name().to_string(),
                elm_assist_lsp::state::RuleInfo {
                    description: r.description(),
                    fixable: false,
                },
            )
        })
        .collect();

    ServerState {
        documents: HashMap::new(),
        project_context: None,
        config: Config::default(),
        rules: all_rules,
        workspace_root: PathBuf::from("/test"),
        all_module_names: Vec::new(),
        rule_descriptions,
    }
}

fn make_state_with_source(source: &str) -> (ServerState, Url) {
    let uri = Url::parse("file:///test/src/Test.elm").or_fail_with("parse URI").unwrap();
    let mut state = make_state();
    state.update_document(&uri, source.to_string(), 1);
    state.rebuild_project_context();
    (state, uri)
}

#[test]
fn lint_detects_unused_import() -> TestResult {
    let source = "module Test exposing (..)\n\nimport Html\n\nx = 1\n";
    let (state, uri) = make_state_with_source(source);

    let errors = analysis::lint_document(&state, &uri);

    let has_unused_import = errors.iter().any(|e| e.rule == "NoUnusedImports");
    check!(has_unused_import).satisfies(is_true()).context(format!(
        "expected NoUnusedImports to fire, got: {:?}",
        errors.iter().map(|e| e.rule).collect::<Vec<_>>()
    ))?;
    Ok(())
}

#[test]
fn lint_detects_debug_log() -> TestResult {
    let source = "module Test exposing (..)\n\nx = Debug.log \"hi\" 1\n";
    let (state, uri) = make_state_with_source(source);

    let errors = analysis::lint_document(&state, &uri);

    let has_debug = errors.iter().any(|e| e.rule == "NoDebug");
    check!(has_debug).satisfies(is_true()).context(format!(
        "expected NoDebug to fire, got: {:?}",
        errors.iter().map(|e| e.rule).collect::<Vec<_>>()
    ))?;
    Ok(())
}

#[test]
fn lint_clean_file_has_no_errors() -> TestResult {
    let source = "module Test exposing (x)\n\n\n{-| A value. -}\nx : Int\nx =\n    1\n";
    let (state, uri) = make_state_with_source(source);

    let errors = analysis::lint_document(&state, &uri);

    let non_project_errors: Vec<_> = errors
        .iter()
        .filter(|e| {
            !matches!(
                e.rule,
                "NoUnusedExports"
                    | "NoUnusedCustomTypeConstructors"
                    | "NoUnusedModules"
                    | "NoMissingDocumentation"
            )
        })
        .collect();

    check!(non_project_errors.is_empty()).satisfies(is_true()).context(format!(
        "expected no non-project errors, got: {:?}",
        non_project_errors
            .iter()
            .map(|e| (e.rule, &e.message))
            .collect::<Vec<_>>()
    ))?;
    Ok(())
}

#[test]
fn lint_unparseable_file_returns_empty_lint_errors() -> TestResult {
    let source = "this is not valid elm at all {{{";
    let (state, uri) = make_state_with_source(source);

    let errors = analysis::lint_document(&state, &uri);
    check!(errors.is_empty())
        .satisfies(is_true())
        .context("expected no lint errors for unparseable file")?;
    Ok(())
}

#[test]
fn unparseable_file_has_parse_errors() -> TestResult {
    let source = "this is not valid elm at all {{{";
    let (state, uri) = make_state_with_source(source);

    let doc = state.documents.get(&uri).or_fail_with("document in state")?;
    check!(!doc.parse_errors.is_empty())
        .satisfies(is_true())
        .context("expected parse errors for invalid source")?;
    Ok(())
}

#[test]
fn parse_recovering_provides_partial_ast() -> TestResult {
    // Valid module header and one valid declaration, with invalid syntax after.
    let source = "module Test exposing (x)\n\n\nx =\n    1\n\n\ny = {{{ invalid\n";
    let (state, uri) = make_state_with_source(source);

    let doc = state.documents.get(&uri).or_fail_with("document in state")?;

    // Should have a partial AST (the valid declaration parsed).
    check!(doc.module.is_some())
        .satisfies(is_true())
        .context("expected partial AST from recovering parse")?;

    // Should have parse errors for the invalid part.
    check!(!doc.parse_errors.is_empty())
        .satisfies(is_true())
        .context("expected parse errors for partially-invalid source")?;

    // Should still be able to lint the valid parts.
    let errors = analysis::lint_document(&state, &uri);
    // The valid code might or might not trigger lint rules, but we shouldn't crash.
    let _ = errors;
    Ok(())
}

#[test]
fn lint_all_open_lints_every_document() -> TestResult {
    let source1 = "module A exposing (..)\n\nimport Html\n\nx = 1\n";
    let source2 = "module B exposing (..)\n\ny = Debug.log \"hi\" 1\n";

    let uri1 = Url::parse("file:///test/src/A.elm").or_fail_with("parse URI")?;
    let uri2 = Url::parse("file:///test/src/B.elm").or_fail_with("parse URI")?;

    let mut state = make_state();
    state.update_document(&uri1, source1.to_string(), 1);
    state.update_document(&uri2, source2.to_string(), 1);
    state.rebuild_project_context();

    let all_results = analysis::lint_all_open(&state);

    check!(all_results.contains_key(&uri1)).satisfies(is_true())?;
    check!(all_results.contains_key(&uri2)).satisfies(is_true())?;

    let a_errors = &all_results[&uri1];
    let b_errors = &all_results[&uri2];

    check!(a_errors.iter().any(|e| e.rule == "NoUnusedImports")).satisfies(is_true())?;
    check!(b_errors.iter().any(|e| e.rule == "NoDebug")).satisfies(is_true())?;
    Ok(())
}

#[test]
fn update_document_detects_import_change() -> TestResult {
    let source_v1 = "module Test exposing (..)\n\nx = 1\n";
    let source_v2 = "module Test exposing (..)\n\nimport Html\n\nx = 1\n";

    let uri = Url::parse("file:///test/src/Test.elm").or_fail_with("parse URI")?;
    let mut state = make_state();

    let needs_rebuild = state.update_document(&uri, source_v1.to_string(), 1);
    check!(needs_rebuild)
        .satisfies(is_true())
        .context("first insert should need rebuild")?;

    let needs_rebuild = state.update_document(&uri, source_v1.to_string(), 2);
    check!(needs_rebuild)
        .satisfies(is_false())
        .context("same source should not need rebuild")?;

    let needs_rebuild = state.update_document(&uri, source_v2.to_string(), 3);
    check!(needs_rebuild)
        .satisfies(is_true())
        .context("adding import should need rebuild")?;
    Ok(())
}

#[test]
fn rule_descriptions_populated() -> TestResult {
    let state = make_state();
    check!(!state.rule_descriptions.is_empty())
        .satisfies(is_true())
        .context("rule descriptions should be populated")?;
    check!(state.rule_descriptions.contains_key("NoUnusedImports"))
        .satisfies(is_true())
        .context("should have NoUnusedImports description")?;
    Ok(())
}
