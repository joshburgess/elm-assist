//! Integration tests for the async command layer.
//!
//! The unit tests in `app.rs` exercise the pure `update` function with
//! synthetic messages. These tests drive the real `command::execute` path
//! end-to-end: write files to a temp dir, run a command, assert the
//! correct messages come back over the mpsc channel.
//!
//! These are the first tests to actually run the lint/parse pipeline
//! through the TUI's async entry point.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::timeout;

use elm_assist_tui::app::{Command, Msg};
use elm_assist_tui::command;
use test_better::ErrorKind;
use test_better::prelude::*;

fn fail(msg: impl Into<String>) -> TestError {
    TestError::new(ErrorKind::Assertion).with_message(msg.into())
}

// ── Test scaffolding ────────────────────────────────────────────────

/// RAII temp project directory. Dropped on test exit.
struct TempProject {
    root: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Result<Self, TestError> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| fail(format!("system time before UNIX_EPOCH: {e}")))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "elm-assist-tui-test-{}-{}-{}",
            name,
            std::process::id(),
            nanos,
        ));
        std::fs::create_dir_all(root.join("src"))
            .map_err(|e| fail(format!("create temp src dir: {e}")))?;
        Ok(Self { root })
    }

    fn src_dir(&self) -> String {
        self.root.join("src").display().to_string()
    }

    fn write(&self, relpath: &str, content: &str) -> Result<(), TestError> {
        let p = self.root.join("src").join(relpath);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| fail(format!("create parent dir: {e}")))?;
        }
        std::fs::write(&p, content).map_err(|e| fail(format!("write {p:?}: {e}")))?;
        Ok(())
    }

    fn path_in_src(&self, relpath: &str) -> String {
        self.root.join("src").join(relpath).display().to_string()
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Drain all messages sent to `rx` within a short window.
/// The command executor calls `spawn_blocking` then returns; messages
/// may arrive slightly after the command future resolves.
async fn drain(rx: &mut mpsc::UnboundedReceiver<Msg>) -> Vec<Msg> {
    let mut out = Vec::new();
    while let Ok(Some(msg)) = timeout(Duration::from_millis(50), rx.recv()).await {
        out.push(msg);
    }
    out
}

/// Short tag for each Msg variant. Used in assertion failure messages
/// because `Msg` does not implement `Debug`.
fn tag(msg: &Msg) -> &'static str {
    match msg {
        Msg::KeyPress(_) => "KeyPress",
        Msg::MouseEvent(_) => "MouseEvent",
        Msg::Quit => "Quit",
        Msg::Tick => "Tick",
        Msg::FileChanged(_) => "FileChanged",
        Msg::StatusError(_) => "StatusError",
        Msg::StatusInfo(_) => "StatusInfo",
        Msg::ClearStatus(_) => "ClearStatus",
        Msg::LintComplete(_) => "LintComplete",
        Msg::DepsComplete { .. } => "DepsComplete",
        Msg::SearchComplete(_) => "SearchComplete",
        Msg::PreviewLoaded { .. } => "PreviewLoaded",
        Msg::ProjectScanned { .. } => "ProjectScanned",
    }
}

fn tags(msgs: &[Msg]) -> Vec<&'static str> {
    msgs.iter().map(tag).collect()
}

/// Minimal valid Elm source for tests that just need the parser to accept something.
const GOOD_ELM: &str = "module Main exposing (..)\n\nx = 1\n";

// ── ScanProject ─────────────────────────────────────────────────────

#[tokio::test]
async fn scan_project_reports_module_counts() -> TestResult {
    let project = TempProject::new("scan-ok")?;
    project.write("A.elm", GOOD_ELM)?;
    project.write("B.elm", "module B exposing (..)\n\ny = 2\n")?;

    let (tx, mut rx) = mpsc::unbounded_channel();
    command::execute(Command::ScanProject, project.src_dir(), tx).await;

    let msgs = drain(&mut rx).await;
    let scanned = msgs
        .iter()
        .find_map(|m| match m {
            Msg::ProjectScanned {
                module_count,
                file_count,
                parse_error_count,
                ..
            } => Some((*module_count, *file_count, *parse_error_count)),
            _ => None,
        })
        .ok_or_else(|| fail(format!("expected Msg::ProjectScanned, got: {:?}", tags(&msgs))))?;

    check!(scanned).satisfies(eq((2, 2, 0))).context("module, file, parse_error counts")?;
    Ok(())
}

#[tokio::test]
async fn scan_project_counts_parse_errors() -> TestResult {
    let project = TempProject::new("scan-parse-err")?;
    project.write("A.elm", GOOD_ELM)?;
    project.write("Broken.elm", "module Broken exposing (..\n\n(((\n")?;

    let (tx, mut rx) = mpsc::unbounded_channel();
    command::execute(Command::ScanProject, project.src_dir(), tx).await;

    let msgs = drain(&mut rx).await;
    let (modules, files, parse_errs) = msgs
        .iter()
        .find_map(|m| match m {
            Msg::ProjectScanned {
                module_count,
                file_count,
                parse_error_count,
                ..
            } => Some((*module_count, *file_count, *parse_error_count)),
            _ => None,
        })
        .ok_or_else(|| fail(format!("expected Msg::ProjectScanned, got: {:?}", tags(&msgs))))?;

    check!(files).satisfies(eq(2)).context("both files discovered")?;
    check!(modules).satisfies(eq(1)).context("only the valid file parses into a module")?;
    check!(parse_errs).satisfies(eq(1)).context("one parse error reported")?;
    Ok(())
}

#[tokio::test]
async fn scan_project_missing_dir_emits_status_error() -> TestResult {
    let (tx, mut rx) = mpsc::unbounded_channel();
    command::execute(
        Command::ScanProject,
        "/definitely/not/a/real/path/xyz".into(),
        tx,
    )
    .await;

    let msgs = drain(&mut rx).await;
    check!(msgs.iter().any(|m| matches!(m, Msg::StatusError(_))))
        .satisfies(is_true())
        .context(format!("expected a StatusError, got: {:?}", tags(&msgs)))?;
    Ok(())
}

// ── RunAnalyses ─────────────────────────────────────────────────────

#[tokio::test]
async fn run_analyses_emits_lint_and_deps() -> TestResult {
    let project = TempProject::new("run-analyses")?;
    project.write("Main.elm", GOOD_ELM)?;

    let (tx, mut rx) = mpsc::unbounded_channel();
    command::execute(Command::RunAnalyses, project.src_dir(), tx).await;

    let msgs = drain(&mut rx).await;
    let has_lint = msgs.iter().any(|m| matches!(m, Msg::LintComplete(_)));
    let has_deps = msgs.iter().any(|m| matches!(m, Msg::DepsComplete { .. }));

    check!(has_lint)
        .satisfies(is_true())
        .context(format!("expected LintComplete, got: {:?}", tags(&msgs)))?;
    check!(has_deps)
        .satisfies(is_true())
        .context(format!("expected DepsComplete, got: {:?}", tags(&msgs)))?;
    Ok(())
}

// ── ApplyFix ────────────────────────────────────────────────────────

#[tokio::test]
async fn apply_fix_writes_when_result_parses() -> TestResult {
    let project = TempProject::new("apply-ok")?;
    let target = project.path_in_src("Target.elm");
    project.write("Target.elm", "module Target exposing (..)\n\nx = 0\n")?;

    let new_source = "module Target exposing (..)\n\nx = 42\n".to_string();
    let (tx, mut rx) = mpsc::unbounded_channel();
    command::execute(
        Command::ApplyFix(target.clone(), new_source.clone()),
        project.src_dir(),
        tx,
    )
    .await;

    let msgs = drain(&mut rx).await;
    check!(!msgs.iter().any(|m| matches!(m, Msg::StatusError(_))))
        .satisfies(is_true())
        .context(format!("valid fix should not emit StatusError, got: {:?}", tags(&msgs)))?;

    let written = std::fs::read_to_string(&target)
        .map_err(|e| fail(format!("file should exist: {e}")))?;
    check!(written).satisfies(eq(new_source)).context("file should contain the fixed source")?;
    Ok(())
}

#[tokio::test]
async fn apply_fix_rejects_invalid_elm_without_writing() -> TestResult {
    let project = TempProject::new("apply-invalid")?;
    let target = project.path_in_src("Target.elm");
    let original = "module Target exposing (..)\n\nx = 0\n";
    project.write("Target.elm", original)?;

    // Missing `exposing` is a parse error.
    let bad_source = "module Target\n\nx = \n".to_string();
    let (tx, mut rx) = mpsc::unbounded_channel();
    command::execute(
        Command::ApplyFix(target.clone(), bad_source),
        project.src_dir(),
        tx,
    )
    .await;

    let msgs = drain(&mut rx).await;
    check!(msgs.iter().any(|m| matches!(m, Msg::StatusError(_))))
        .satisfies(is_true())
        .context(format!("invalid fix should emit StatusError, got: {:?}", tags(&msgs)))?;

    let on_disk = std::fs::read_to_string(&target)
        .map_err(|e| fail(format!("file should still exist: {e}")))?;
    check!(on_disk.as_str())
        .satisfies(eq(original))
        .context("file must be untouched when fix produces invalid Elm")?;
    Ok(())
}

// ── ExportLintJson ──────────────────────────────────────────────────

#[tokio::test]
async fn export_lint_json_writes_file_and_status_info() -> TestResult {
    use elm_ast::span::{Position, Span};
    use elm_lint::rule::{LintError, Severity};

    let project = TempProject::new("export-json")?;

    let err = LintError {
        rule: "NoDebug",
        severity: Severity::Warning,
        message: "debug call found".into(),
        span: Span {
            start: Position {
                offset: 0,
                line: 1,
                column: 1,
            },
            end: Position {
                offset: 5,
                line: 1,
                column: 6,
            },
        },
        fix: None,
    };

    let payload = Arc::new(vec![("src/A.elm".to_string(), err)]);
    let (tx, mut rx) = mpsc::unbounded_channel();
    command::execute(Command::ExportLintJson(payload), project.src_dir(), tx).await;

    let msgs = drain(&mut rx).await;
    check!(msgs.iter().any(|m| matches!(m, Msg::StatusInfo(_))))
        .satisfies(is_true())
        .context(format!("expected StatusInfo on successful export, got: {:?}", tags(&msgs)))?;

    // File resolves to parent-of-src_dir / elm-assist-lint.json.
    let out_path = project.root.join("elm-assist-lint.json");
    let json = std::fs::read_to_string(&out_path)
        .map_err(|e| fail(format!("json file should exist: {e}")))?;
    let parsed: serde_json::Value = serde_json::from_str(&json)
        .map_err(|e| fail(format!("json must be valid: {e}")))?;
    let arr = parsed.as_array()
        .ok_or_else(|| fail("root must be array"))?;
    check!(arr.len()).satisfies(eq(1))?;
    check!(arr[0]["rule"].clone()).satisfies(eq(serde_json::json!("NoDebug")))?;
    check!(arr[0]["severity"].clone()).satisfies(eq(serde_json::json!("warning")))?;
    check!(arr[0]["message"].clone()).satisfies(eq(serde_json::json!("debug call found")))?;
    check!(arr[0]["file"].clone()).satisfies(eq(serde_json::json!("src/A.elm")))?;
    check!(arr[0]["line"].clone()).satisfies(eq(serde_json::json!(1)))?;
    check!(arr[0]["fixable"].clone()).satisfies(eq(serde_json::json!(false)))?;
    Ok(())
}

// ── Batch ───────────────────────────────────────────────────────────

#[tokio::test]
async fn batch_runs_sub_commands_sequentially() -> TestResult {
    let project = TempProject::new("batch")?;
    let a = project.path_in_src("A.elm");
    let b = project.path_in_src("B.elm");
    project.write("A.elm", "module A exposing (..)\n\nx = 0\n")?;
    project.write("B.elm", "module B exposing (..)\n\ny = 0\n")?;

    let a_new = "module A exposing (..)\n\nx = 1\n".to_string();
    let b_new = "module B exposing (..)\n\ny = 2\n".to_string();

    let batch = Command::Batch(vec![
        Command::ApplyFix(a.clone(), a_new.clone()),
        Command::ApplyFix(b.clone(), b_new.clone()),
    ]);

    let (tx, mut rx) = mpsc::unbounded_channel();
    command::execute(batch, project.src_dir(), tx).await;
    let _ = drain(&mut rx).await;

    check!(std::fs::read_to_string(&a).map_err(|e| fail(format!("read a: {e}")))?)
        .satisfies(eq(a_new))?;
    check!(std::fs::read_to_string(&b).map_err(|e| fail(format!("read b: {e}")))?)
        .satisfies(eq(b_new))?;
    Ok(())
}
