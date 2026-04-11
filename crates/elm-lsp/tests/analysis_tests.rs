use std::collections::HashMap;
use std::path::PathBuf;

use tower_lsp::lsp_types::Url;

use elm_lint::config::Config;
use elm_lint::rules;

use elm_lsp::analysis;
use elm_lsp::state::ServerState;

fn make_state() -> ServerState {
    let all_rules = rules::all_rules();
    let rule_descriptions = all_rules
        .iter()
        .map(|r| {
            (
                r.name().to_string(),
                elm_lsp::state::RuleInfo {
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
    let uri = Url::parse("file:///test/src/Test.elm").unwrap();
    let mut state = make_state();
    state.update_document(&uri, source.to_string(), 1);
    state.rebuild_project_context();
    (state, uri)
}

#[test]
fn lint_detects_unused_import() {
    let source = "module Test exposing (..)\n\nimport Html\n\nx = 1\n";
    let (state, uri) = make_state_with_source(source);

    let errors = analysis::lint_document(&state, &uri);

    let has_unused_import = errors.iter().any(|e| e.rule == "NoUnusedImports");
    assert!(
        has_unused_import,
        "expected NoUnusedImports to fire, got: {:?}",
        errors.iter().map(|e| e.rule).collect::<Vec<_>>()
    );
}

#[test]
fn lint_detects_debug_log() {
    let source = "module Test exposing (..)\n\nx = Debug.log \"hi\" 1\n";
    let (state, uri) = make_state_with_source(source);

    let errors = analysis::lint_document(&state, &uri);

    let has_debug = errors.iter().any(|e| e.rule == "NoDebug");
    assert!(
        has_debug,
        "expected NoDebug to fire, got: {:?}",
        errors.iter().map(|e| e.rule).collect::<Vec<_>>()
    );
}

#[test]
fn lint_clean_file_has_no_errors() {
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

    assert!(
        non_project_errors.is_empty(),
        "expected no non-project errors, got: {:?}",
        non_project_errors
            .iter()
            .map(|e| (e.rule, &e.message))
            .collect::<Vec<_>>()
    );
}

#[test]
fn lint_unparseable_file_returns_empty_lint_errors() {
    let source = "this is not valid elm at all {{{";
    let (state, uri) = make_state_with_source(source);

    let errors = analysis::lint_document(&state, &uri);
    assert!(
        errors.is_empty(),
        "expected no lint errors for unparseable file"
    );
}

#[test]
fn unparseable_file_has_parse_errors() {
    let source = "this is not valid elm at all {{{";
    let (state, uri) = make_state_with_source(source);

    let doc = state.documents.get(&uri).unwrap();
    assert!(
        !doc.parse_errors.is_empty(),
        "expected parse errors for invalid source"
    );
}

#[test]
fn parse_recovering_provides_partial_ast() {
    // Valid module header and one valid declaration, with invalid syntax after.
    let source = "module Test exposing (x)\n\n\nx =\n    1\n\n\ny = {{{ invalid\n";
    let (state, uri) = make_state_with_source(source);

    let doc = state.documents.get(&uri).unwrap();

    // Should have a partial AST (the valid declaration parsed).
    assert!(doc.module.is_some(), "expected partial AST from recovering parse");

    // Should have parse errors for the invalid part.
    assert!(
        !doc.parse_errors.is_empty(),
        "expected parse errors for partially-invalid source"
    );

    // Should still be able to lint the valid parts.
    let errors = analysis::lint_document(&state, &uri);
    // The valid code might or might not trigger lint rules, but we shouldn't crash.
    let _ = errors;
}

#[test]
fn lint_all_open_lints_every_document() {
    let source1 = "module A exposing (..)\n\nimport Html\n\nx = 1\n";
    let source2 = "module B exposing (..)\n\ny = Debug.log \"hi\" 1\n";

    let uri1 = Url::parse("file:///test/src/A.elm").unwrap();
    let uri2 = Url::parse("file:///test/src/B.elm").unwrap();

    let mut state = make_state();
    state.update_document(&uri1, source1.to_string(), 1);
    state.update_document(&uri2, source2.to_string(), 1);
    state.rebuild_project_context();

    let all_results = analysis::lint_all_open(&state);

    assert!(all_results.contains_key(&uri1));
    assert!(all_results.contains_key(&uri2));

    let a_errors = &all_results[&uri1];
    let b_errors = &all_results[&uri2];

    assert!(a_errors.iter().any(|e| e.rule == "NoUnusedImports"));
    assert!(b_errors.iter().any(|e| e.rule == "NoDebug"));
}

#[test]
fn update_document_detects_import_change() {
    let source_v1 = "module Test exposing (..)\n\nx = 1\n";
    let source_v2 = "module Test exposing (..)\n\nimport Html\n\nx = 1\n";

    let uri = Url::parse("file:///test/src/Test.elm").unwrap();
    let mut state = make_state();

    let needs_rebuild = state.update_document(&uri, source_v1.to_string(), 1);
    assert!(needs_rebuild, "first insert should need rebuild");

    let needs_rebuild = state.update_document(&uri, source_v1.to_string(), 2);
    assert!(!needs_rebuild, "same source should not need rebuild");

    let needs_rebuild = state.update_document(&uri, source_v2.to_string(), 3);
    assert!(needs_rebuild, "adding import should need rebuild");
}

#[test]
fn rule_descriptions_populated() {
    let state = make_state();
    assert!(
        !state.rule_descriptions.is_empty(),
        "rule descriptions should be populated"
    );
    assert!(
        state.rule_descriptions.contains_key("NoUnusedImports"),
        "should have NoUnusedImports description"
    );
}
