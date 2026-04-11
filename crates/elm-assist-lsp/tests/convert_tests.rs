use tower_lsp::lsp_types::{self, DiagnosticSeverity, NumberOrString, Url};

use elm_ast::parse::ParseError;
use elm_ast::span::{Position, Span};
use elm_lint::rule::{Edit, Fix, LintError, Severity};

use elm_assist_lsp::convert;

fn span(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Span {
    Span {
        start: Position {
            offset: 0,
            line: start_line,
            column: start_col,
        },
        end: Position {
            offset: 10,
            line: end_line,
            column: end_col,
        },
    }
}

fn make_error(severity: Severity, start_line: u32, start_col: u32) -> LintError {
    LintError {
        rule: "TestRule",
        severity,
        message: "test message".into(),
        span: span(start_line, start_col, start_line, start_col + 5),
        fix: None,
    }
}

// ── span_to_range ─────────────────────────────────────────────────

#[test]
fn span_to_range_converts_1based_to_0based() {
    let s = span(1, 1, 1, 10);
    let r = convert::span_to_range(&s);
    assert_eq!(r.start.line, 0);
    assert_eq!(r.start.character, 0);
    assert_eq!(r.end.line, 0);
    assert_eq!(r.end.character, 9);
}

#[test]
fn span_to_range_multiline() {
    let s = span(5, 3, 10, 15);
    let r = convert::span_to_range(&s);
    assert_eq!(r.start.line, 4);
    assert_eq!(r.start.character, 2);
    assert_eq!(r.end.line, 9);
    assert_eq!(r.end.character, 14);
}

#[test]
fn span_to_range_zero_width() {
    let s = span(3, 7, 3, 7);
    let r = convert::span_to_range(&s);
    assert_eq!(r.start, r.end);
    assert_eq!(r.start.line, 2);
    assert_eq!(r.start.character, 6);
}

// ── severity_to_lsp ──────────────────────────────────────────────

#[test]
fn severity_error_maps_to_lsp_error() {
    assert_eq!(
        convert::severity_to_lsp(Severity::Error),
        DiagnosticSeverity::ERROR
    );
}

#[test]
fn severity_warning_maps_to_lsp_warning() {
    assert_eq!(
        convert::severity_to_lsp(Severity::Warning),
        DiagnosticSeverity::WARNING
    );
}

// ── lint_error_to_diagnostic ─────────────────────────────────────

#[test]
fn diagnostic_has_correct_fields() {
    let error = make_error(Severity::Warning, 5, 3);
    let diag = convert::lint_error_to_diagnostic(&error);

    assert_eq!(diag.severity, Some(DiagnosticSeverity::WARNING));
    assert_eq!(diag.source, Some("elm-lint".into()));
    assert_eq!(diag.code, Some(NumberOrString::String("TestRule".into())));
    assert_eq!(diag.message, "test message");
    assert_eq!(diag.range.start.line, 4);
    assert_eq!(diag.range.start.character, 2);
}

#[test]
fn lint_errors_to_diagnostics_maps_all() {
    let errors = vec![
        make_error(Severity::Error, 1, 1),
        make_error(Severity::Warning, 2, 1),
    ];
    let diags = convert::lint_errors_to_diagnostics(&errors);
    assert_eq!(diags.len(), 2);
    assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
    assert_eq!(diags[1].severity, Some(DiagnosticSeverity::WARNING));
}

// ── edit_to_text_edit ────────────────────────────────────────────

#[test]
fn replace_edit_to_text_edit() {
    let edit = Edit::Replace {
        span: span(1, 1, 1, 5),
        replacement: "new".into(),
    };
    let te = convert::edit_to_text_edit(&edit);
    assert_eq!(te.new_text, "new");
    assert_eq!(te.range.start.line, 0);
    assert_eq!(te.range.start.character, 0);
    assert_eq!(te.range.end.line, 0);
    assert_eq!(te.range.end.character, 4);
}

#[test]
fn insert_after_edit_to_text_edit() {
    let edit = Edit::InsertAfter {
        span: span(3, 10, 3, 10),
        text: " inserted".into(),
    };
    let te = convert::edit_to_text_edit(&edit);
    assert_eq!(te.new_text, " inserted");
    // Insert point should be at span.end (0-based).
    assert_eq!(te.range.start.line, 2);
    assert_eq!(te.range.start.character, 9);
    assert_eq!(te.range.start, te.range.end);
}

#[test]
fn remove_edit_to_text_edit() {
    let edit = Edit::Remove {
        span: span(2, 5, 2, 15),
    };
    let te = convert::edit_to_text_edit(&edit);
    assert_eq!(te.new_text, "");
    assert_eq!(te.range.start.line, 1);
    assert_eq!(te.range.start.character, 4);
    assert_eq!(te.range.end.line, 1);
    assert_eq!(te.range.end.character, 14);
}

// ── fix_to_code_action ───────────────────────────────────────────

#[test]
fn no_fix_returns_none() {
    let error = make_error(Severity::Warning, 1, 1);
    let uri = Url::parse("file:///test.elm").unwrap();
    assert!(convert::fix_to_code_action(&uri, &error).is_none());
}

#[test]
fn fix_to_code_action_creates_quickfix() {
    let error = LintError {
        rule: "TestRule",
        severity: Severity::Warning,
        message: "test".into(),
        span: span(1, 1, 1, 5),
        fix: Some(Fix::replace(span(1, 1, 1, 5), "replacement".into())),
    };
    let uri = Url::parse("file:///test.elm").unwrap();
    let action = convert::fix_to_code_action(&uri, &error).unwrap();

    assert_eq!(action.kind, Some(lsp_types::CodeActionKind::QUICKFIX));
    assert!(action.title.contains("TestRule"));
    assert!(action.edit.is_some());

    let edit = action.edit.unwrap();
    let changes = edit.changes.unwrap();
    let edits = changes.get(&uri).unwrap();
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].new_text, "replacement");
}

#[test]
fn fix_with_multiple_edits() {
    let error = LintError {
        rule: "TestRule",
        severity: Severity::Warning,
        message: "test".into(),
        span: span(1, 1, 1, 5),
        fix: Some(Fix {
            edits: vec![
                Edit::Replace {
                    span: span(1, 1, 1, 5),
                    replacement: "a".into(),
                },
                Edit::Remove {
                    span: span(2, 1, 2, 10),
                },
            ],
        }),
    };
    let uri = Url::parse("file:///test.elm").unwrap();
    let action = convert::fix_to_code_action(&uri, &error).unwrap();

    let changes = action.edit.unwrap().changes.unwrap();
    let edits = changes.get(&uri).unwrap();
    assert_eq!(edits.len(), 2);
}

// ── parse_error_to_diagnostic ────────────────────────────────────

#[test]
fn parse_error_diagnostic_has_correct_fields() {
    let error = ParseError {
        message: "unexpected token".into(),
        span: span(3, 5, 3, 10),
    };
    let diag = convert::parse_error_to_diagnostic(&error);

    assert_eq!(diag.severity, Some(DiagnosticSeverity::ERROR));
    assert_eq!(diag.source, Some("elm-lint".into()));
    assert_eq!(
        diag.code,
        Some(NumberOrString::String("parse-error".into()))
    );
    assert_eq!(diag.message, "unexpected token");
    assert_eq!(diag.range.start.line, 2);
    assert_eq!(diag.range.start.character, 4);
}

#[test]
fn parse_errors_to_diagnostics_maps_all() {
    let errors = vec![
        ParseError {
            message: "error 1".into(),
            span: span(1, 1, 1, 5),
        },
        ParseError {
            message: "error 2".into(),
            span: span(2, 1, 2, 5),
        },
    ];
    let diags = convert::parse_errors_to_diagnostics(&errors);
    assert_eq!(diags.len(), 2);
    assert_eq!(diags[0].message, "error 1");
    assert_eq!(diags[1].message, "error 2");
}
