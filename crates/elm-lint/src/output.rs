use std::collections::HashMap;
use std::io::{self, IsTerminal, Write};

use ariadne::{Color, Label, Report, ReportKind, Source};

use crate::rule::{LintError, Severity};

/// Output format for lint results.
pub enum OutputFormat {
    /// Colored diagnostics with source context (ariadne).
    Rich,
    /// Plain single-line-per-error format.
    Plain,
    /// Machine-readable JSON.
    Json,
}

/// Determines the output format based on CLI flags and TTY detection.
pub fn resolve_format(json: bool, color: bool, no_color: bool) -> OutputFormat {
    if json {
        OutputFormat::Json
    } else if no_color {
        OutputFormat::Plain
    } else if color || io::stderr().is_terminal() {
        OutputFormat::Rich
    } else {
        OutputFormat::Plain
    }
}

/// Report all lint errors to stdout/stderr.
pub fn report(
    format: &OutputFormat,
    file_errors: &HashMap<String, Vec<LintError>>,
    sources: &HashMap<String, String>,
    files_checked: usize,
    rules_active: usize,
) {
    match format {
        OutputFormat::Rich => report_rich(file_errors, sources),
        OutputFormat::Plain => report_plain(file_errors),
        OutputFormat::Json => report_json(file_errors, files_checked, rules_active),
    }
}

/// Print a summary of findings.
pub fn report_summary(format: &OutputFormat, file_errors: &HashMap<String, Vec<LintError>>) {
    if matches!(format, OutputFormat::Json) {
        return; // JSON output includes summary inline.
    }

    let total: usize = file_errors.values().map(|v| v.len()).sum();
    if total == 0 {
        println!("No lint errors found.");
        return;
    }

    let fixable: usize = file_errors
        .values()
        .flat_map(|v| v.iter())
        .filter(|e| e.fix.is_some())
        .count();

    let errors: usize = file_errors
        .values()
        .flat_map(|v| v.iter())
        .filter(|e| e.severity == Severity::Error)
        .count();

    let warnings = total - errors;

    println!();
    println!(
        "{total} findings ({errors} errors, {warnings} warnings) in {} files",
        file_errors.len()
    );
    if fixable > 0 {
        println!("  {fixable} auto-fixable (run with --fix or --fix-all)");
    }

    // Per-rule breakdown.
    let mut by_rule: HashMap<&str, usize> = HashMap::new();
    for errors in file_errors.values() {
        for err in errors {
            *by_rule.entry(err.rule).or_default() += 1;
        }
    }
    let mut rule_counts: Vec<_> = by_rule.into_iter().collect();
    rule_counts.sort_by_key(|(_, v)| std::cmp::Reverse(*v));
    for (rule, count) in &rule_counts {
        println!("  {count:>4} {rule}");
    }
}

// ── Rich output (ariadne) ──────────────────────────────────────────

fn report_rich(file_errors: &HashMap<String, Vec<LintError>>, sources: &HashMap<String, String>) {
    let mut paths: Vec<&String> = file_errors.keys().collect();
    paths.sort();

    for path in paths {
        let errors = &file_errors[path];
        let Some(source) = sources.get(path) else {
            continue;
        };

        let mut sorted = errors.clone();
        sorted.sort_by_key(|e| (e.span.start.line, e.span.start.column));

        for err in &sorted {
            let kind = match err.severity {
                Severity::Error => ReportKind::Error,
                Severity::Warning => ReportKind::Warning,
            };

            let color = match err.severity {
                Severity::Error => Color::Red,
                Severity::Warning => Color::Yellow,
            };

            let span = (path.as_str(), err.span.start.offset..err.span.end.offset);

            let mut builder = Report::build(kind, span.clone())
                .with_code(err.rule)
                .with_message(&err.message)
                .with_label(Label::new(span).with_color(color));

            if err.fix.is_some() {
                builder = builder.with_note("auto-fixable (run with --fix)");
            }

            builder
                .finish()
                .eprint((path.as_str(), Source::from(source.as_str())))
                .ok();
        }
    }
}

// ── Plain output ───────────────────────────────────────────────────

fn report_plain(file_errors: &HashMap<String, Vec<LintError>>) {
    let mut paths: Vec<&String> = file_errors.keys().collect();
    paths.sort();

    for path in paths {
        let errors = &file_errors[path];
        let mut sorted = errors.clone();
        sorted.sort_by_key(|e| (e.span.start.line, e.span.start.column));

        for err in &sorted {
            let severity = match err.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            };
            let fix_marker = if err.fix.is_some() { " (fixable)" } else { "" };
            println!(
                "{}:{}:{}: {}: [{}] {}{}",
                path,
                err.span.start.line,
                err.span.start.column,
                severity,
                err.rule,
                err.message,
                fix_marker
            );
        }
    }
}

// ── JSON output ────────────────────────────────────────────────────

fn report_json(
    file_errors: &HashMap<String, Vec<LintError>>,
    files_checked: usize,
    rules_active: usize,
) {
    let mut diagnostics = Vec::new();

    let mut paths: Vec<&String> = file_errors.keys().collect();
    paths.sort();

    let mut total_errors = 0usize;
    let mut total_warnings = 0usize;
    let mut total_fixable = 0usize;

    for path in paths {
        for err in &file_errors[path] {
            match err.severity {
                Severity::Error => total_errors += 1,
                Severity::Warning => total_warnings += 1,
            }
            if err.fix.is_some() {
                total_fixable += 1;
            }
            diagnostics.push(JsonDiagnostic {
                file: path.clone(),
                rule: err.rule.to_string(),
                severity: match err.severity {
                    Severity::Error => "error".into(),
                    Severity::Warning => "warning".into(),
                },
                message: err.message.clone(),
                start: JsonPosition {
                    line: err.span.start.line,
                    column: err.span.start.column,
                    offset: err.span.start.offset,
                },
                end: JsonPosition {
                    line: err.span.end.line,
                    column: err.span.end.column,
                    offset: err.span.end.offset,
                },
                fixable: err.fix.is_some(),
            });
        }
    }

    let total = total_errors + total_warnings;

    let output = JsonOutput {
        version: 1,
        files_checked,
        rules_active,
        diagnostics,
        summary: JsonSummary {
            total,
            errors: total_errors,
            warnings: total_warnings,
            fixable: total_fixable,
        },
    };

    let stdout = io::stdout();
    let mut handle = stdout.lock();
    serde_json::to_writer_pretty(&mut handle, &output).ok();
    handle.write_all(b"\n").ok();
}

#[derive(serde::Serialize)]
struct JsonOutput {
    version: u32,
    files_checked: usize,
    rules_active: usize,
    diagnostics: Vec<JsonDiagnostic>,
    summary: JsonSummary,
}

#[derive(serde::Serialize)]
struct JsonDiagnostic {
    file: String,
    rule: String,
    severity: String,
    message: String,
    start: JsonPosition,
    end: JsonPosition,
    fixable: bool,
}

#[derive(serde::Serialize)]
struct JsonPosition {
    line: u32,
    column: u32,
    offset: usize,
}

#[derive(serde::Serialize)]
struct JsonSummary {
    total: usize,
    errors: usize,
    warnings: usize,
    fixable: usize,
}
