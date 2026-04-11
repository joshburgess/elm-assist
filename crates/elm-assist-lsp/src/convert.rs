use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range,
    TextEdit, Url, WorkspaceEdit,
};

use elm_ast::parse::ParseError;
use elm_ast::span::Span;
use elm_lint::rule::{Edit, LintError, Severity};

/// Convert an elm-ast Span to an LSP Range.
/// elm-ast uses 1-based line/column; LSP uses 0-based.
pub fn span_to_range(span: &Span) -> Range {
    Range {
        start: Position {
            line: span.start.line.saturating_sub(1),
            character: span.start.column.saturating_sub(1),
        },
        end: Position {
            line: span.end.line.saturating_sub(1),
            character: span.end.column.saturating_sub(1),
        },
    }
}

/// Convert elm-lint Severity to LSP DiagnosticSeverity.
pub fn severity_to_lsp(severity: Severity) -> DiagnosticSeverity {
    match severity {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
    }
}

/// Convert a LintError to an LSP Diagnostic.
pub fn lint_error_to_diagnostic(error: &LintError) -> Diagnostic {
    Diagnostic {
        range: span_to_range(&error.span),
        severity: Some(severity_to_lsp(error.severity)),
        code: Some(NumberOrString::String(error.rule.to_string())),
        source: Some("elm-lint".into()),
        message: error.message.clone(),
        ..Default::default()
    }
}

/// Convert a list of LintErrors to LSP Diagnostics.
pub fn lint_errors_to_diagnostics(errors: &[LintError]) -> Vec<Diagnostic> {
    errors.iter().map(lint_error_to_diagnostic).collect()
}

/// Convert a ParseError to an LSP Diagnostic.
pub fn parse_error_to_diagnostic(error: &ParseError) -> Diagnostic {
    Diagnostic {
        range: span_to_range(&error.span),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String("parse-error".into())),
        source: Some("elm-lint".into()),
        message: error.message.clone(),
        ..Default::default()
    }
}

/// Convert a list of ParseErrors to LSP Diagnostics.
pub fn parse_errors_to_diagnostics(errors: &[ParseError]) -> Vec<Diagnostic> {
    errors.iter().map(parse_error_to_diagnostic).collect()
}

/// Convert a single Edit to an LSP TextEdit.
pub fn edit_to_text_edit(edit: &Edit) -> TextEdit {
    match edit {
        Edit::Replace { span, replacement } => TextEdit {
            range: span_to_range(span),
            new_text: replacement.clone(),
        },
        Edit::InsertAfter { span, text } => {
            let pos = Position {
                line: span.end.line.saturating_sub(1),
                character: span.end.column.saturating_sub(1),
            };
            TextEdit {
                range: Range {
                    start: pos,
                    end: pos,
                },
                new_text: text.clone(),
            }
        }
        Edit::Remove { span } => TextEdit {
            range: span_to_range(span),
            new_text: String::new(),
        },
    }
}

/// Convert a fixable LintError to an LSP CodeAction. Returns None if no fix.
pub fn fix_to_code_action(uri: &Url, error: &LintError) -> Option<CodeAction> {
    let fix = error.fix.as_ref()?;

    let text_edits: Vec<TextEdit> = fix.edits.iter().map(edit_to_text_edit).collect();

    let mut changes = std::collections::HashMap::new();
    changes.insert(uri.clone(), text_edits);

    Some(CodeAction {
        title: format!("Fix: {} ({})", error.message, error.rule),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![lint_error_to_diagnostic(error)]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        ..Default::default()
    })
}
