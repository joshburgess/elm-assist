use tower_lsp::lsp_types::{self, DiagnosticSeverity, NumberOrString, Url};

use elm_ast::parse::ParseError;
use elm_ast::span::{Position, Span};
use elm_lint::rule::{Edit, Fix, LintError, Severity};

use elm_assist_lsp::convert;
use test_better::ErrorKind;
use test_better::prelude::*;

fn fail(msg: impl Into<String>) -> TestError {
    TestError::new(ErrorKind::Assertion).with_message(msg.into())
}

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
fn span_to_range_converts_1based_to_0based() -> TestResult {
    let s = span(1, 1, 1, 10);
    let r = convert::span_to_range(&s);
    check!(r.start.line).satisfies(eq(0))?;
    check!(r.start.character).satisfies(eq(0))?;
    check!(r.end.line).satisfies(eq(0))?;
    check!(r.end.character).satisfies(eq(9))?;
    Ok(())
}

#[test]
fn span_to_range_multiline() -> TestResult {
    let s = span(5, 3, 10, 15);
    let r = convert::span_to_range(&s);
    check!(r.start.line).satisfies(eq(4))?;
    check!(r.start.character).satisfies(eq(2))?;
    check!(r.end.line).satisfies(eq(9))?;
    check!(r.end.character).satisfies(eq(14))?;
    Ok(())
}

#[test]
fn span_to_range_zero_width() -> TestResult {
    let s = span(3, 7, 3, 7);
    let r = convert::span_to_range(&s);
    check!(r.start).satisfies(eq(r.end))?;
    check!(r.start.line).satisfies(eq(2))?;
    check!(r.start.character).satisfies(eq(6))?;
    Ok(())
}

// ── severity_to_lsp ──────────────────────────────────────────────

#[test]
fn severity_error_maps_to_lsp_error() -> TestResult {
    check!(convert::severity_to_lsp(Severity::Error)).satisfies(eq(DiagnosticSeverity::ERROR))?;
    Ok(())
}

#[test]
fn severity_warning_maps_to_lsp_warning() -> TestResult {
    check!(convert::severity_to_lsp(Severity::Warning))
        .satisfies(eq(DiagnosticSeverity::WARNING))?;
    Ok(())
}

// ── lint_error_to_diagnostic ─────────────────────────────────────

#[test]
fn diagnostic_has_correct_fields() -> TestResult {
    let error = make_error(Severity::Warning, 5, 3);
    let diag = convert::lint_error_to_diagnostic(&error);

    check!(diag.severity).satisfies(eq(Some(DiagnosticSeverity::WARNING)))?;
    check!(diag.source).satisfies(eq(Some("elm-assist".into())))?;
    check!(diag.code).satisfies(eq(Some(NumberOrString::String("TestRule".into()))))?;
    check!(diag.message.as_str()).satisfies(eq("test message"))?;
    check!(diag.range.start.line).satisfies(eq(4))?;
    check!(diag.range.start.character).satisfies(eq(2))?;
    Ok(())
}

#[test]
fn lint_errors_to_diagnostics_maps_all() -> TestResult {
    let errors = vec![
        make_error(Severity::Error, 1, 1),
        make_error(Severity::Warning, 2, 1),
    ];
    let diags = convert::lint_errors_to_diagnostics(&errors);
    check!(diags.len()).satisfies(eq(2))?;
    check!(diags[0].severity).satisfies(eq(Some(DiagnosticSeverity::ERROR)))?;
    check!(diags[1].severity).satisfies(eq(Some(DiagnosticSeverity::WARNING)))?;
    Ok(())
}

// ── edit_to_text_edit ────────────────────────────────────────────

#[test]
fn replace_edit_to_text_edit() -> TestResult {
    let edit = Edit::Replace {
        span: span(1, 1, 1, 5),
        replacement: "new".into(),
    };
    let te = convert::edit_to_text_edit(&edit);
    check!(te.new_text.as_str()).satisfies(eq("new"))?;
    check!(te.range.start.line).satisfies(eq(0))?;
    check!(te.range.start.character).satisfies(eq(0))?;
    check!(te.range.end.line).satisfies(eq(0))?;
    check!(te.range.end.character).satisfies(eq(4))?;
    Ok(())
}

#[test]
fn insert_after_edit_to_text_edit() -> TestResult {
    let edit = Edit::InsertAfter {
        span: span(3, 10, 3, 10),
        text: " inserted".into(),
    };
    let te = convert::edit_to_text_edit(&edit);
    check!(te.new_text.as_str()).satisfies(eq(" inserted"))?;
    // Insert point should be at span.end (0-based).
    check!(te.range.start.line).satisfies(eq(2))?;
    check!(te.range.start.character).satisfies(eq(9))?;
    check!(te.range.start).satisfies(eq(te.range.end))?;
    Ok(())
}

#[test]
fn remove_edit_to_text_edit() -> TestResult {
    let edit = Edit::Remove {
        span: span(2, 5, 2, 15),
    };
    let te = convert::edit_to_text_edit(&edit);
    check!(te.new_text.as_str()).satisfies(eq(""))?;
    check!(te.range.start.line).satisfies(eq(1))?;
    check!(te.range.start.character).satisfies(eq(4))?;
    check!(te.range.end.line).satisfies(eq(1))?;
    check!(te.range.end.character).satisfies(eq(14))?;
    Ok(())
}

// ── fix_to_code_action ───────────────────────────────────────────

#[test]
fn no_fix_returns_none() -> TestResult {
    let error = make_error(Severity::Warning, 1, 1);
    let uri = Url::parse("file:///test.elm").or_fail_with("parse URI")?;
    check!(convert::fix_to_code_action(&uri, &error).is_none()).satisfies(is_true())?;
    Ok(())
}

#[test]
fn fix_to_code_action_creates_quickfix() -> TestResult {
    let error = LintError {
        rule: "TestRule",
        severity: Severity::Warning,
        message: "test".into(),
        span: span(1, 1, 1, 5),
        fix: Some(Fix::replace(span(1, 1, 1, 5), "replacement".into())),
    };
    let uri = Url::parse("file:///test.elm").or_fail_with("parse URI")?;
    let action = convert::fix_to_code_action(&uri, &error)
        .ok_or_else(|| fail("expected Some code action"))?;

    check!(action.kind).satisfies(eq(Some(lsp_types::CodeActionKind::QUICKFIX)))?;
    check!(action.title.as_str()).satisfies(contains_str("TestRule"))?;
    check!(action.edit.is_some()).satisfies(is_true())?;

    let edit = action.edit.ok_or_else(|| fail("expected edit"))?;
    let changes = edit.changes.ok_or_else(|| fail("expected changes"))?;
    let edits = changes
        .get(&uri)
        .ok_or_else(|| fail("expected edits for uri"))?;
    check!(edits.len()).satisfies(eq(1))?;
    check!(edits[0].new_text.as_str()).satisfies(eq("replacement"))?;
    Ok(())
}

#[test]
fn fix_with_multiple_edits() -> TestResult {
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
    let uri = Url::parse("file:///test.elm").or_fail_with("parse URI")?;
    let action = convert::fix_to_code_action(&uri, &error)
        .ok_or_else(|| fail("expected Some code action"))?;

    let changes = action
        .edit
        .ok_or_else(|| fail("expected edit"))?
        .changes
        .ok_or_else(|| fail("expected changes"))?;
    let edits = changes
        .get(&uri)
        .ok_or_else(|| fail("expected edits for uri"))?;
    check!(edits.len()).satisfies(eq(2))?;
    Ok(())
}

// ── parse_error_to_diagnostic ────────────────────────────────────

#[test]
fn parse_error_diagnostic_has_correct_fields() -> TestResult {
    let error = ParseError {
        message: "unexpected token".into(),
        span: span(3, 5, 3, 10),
    };
    let diag = convert::parse_error_to_diagnostic(&error);

    check!(diag.severity).satisfies(eq(Some(DiagnosticSeverity::ERROR)))?;
    check!(diag.source).satisfies(eq(Some("elm-assist".into())))?;
    check!(diag.code).satisfies(eq(Some(NumberOrString::String("parse-error".into()))))?;
    check!(diag.message.as_str()).satisfies(eq("unexpected token"))?;
    check!(diag.range.start.line).satisfies(eq(2))?;
    check!(diag.range.start.character).satisfies(eq(4))?;
    Ok(())
}

#[test]
fn parse_errors_to_diagnostics_maps_all() -> TestResult {
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
    check!(diags.len()).satisfies(eq(2))?;
    check!(diags[0].message.as_str()).satisfies(eq("error 1"))?;
    check!(diags[1].message.as_str()).satisfies(eq("error 2"))?;
    Ok(())
}
