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

// ── Test scaffolding ────────────────────────────────────────────────

/// RAII temp project directory. Dropped on test exit.
struct TempProject {
    root: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "elm-assist-tui-test-{}-{}-{}",
            name,
            std::process::id(),
            nanos,
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        Self { root }
    }

    fn src_dir(&self) -> String {
        self.root.join("src").display().to_string()
    }

    fn write(&self, relpath: &str, content: &str) {
        let p = self.root.join("src").join(relpath);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(p, content).unwrap();
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
async fn scan_project_reports_module_counts() {
    let project = TempProject::new("scan-ok");
    project.write("A.elm", GOOD_ELM);
    project.write("B.elm", "module B exposing (..)\n\ny = 2\n");

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
        .unwrap_or_else(|| panic!("expected Msg::ProjectScanned, got: {:?}", tags(&msgs)));

    assert_eq!(scanned, (2, 2, 0), "module, file, parse_error counts");
}

#[tokio::test]
async fn scan_project_counts_parse_errors() {
    let project = TempProject::new("scan-parse-err");
    project.write("A.elm", GOOD_ELM);
    project.write("Broken.elm", "module Broken exposing (..\n\n(((\n");

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
        .unwrap_or_else(|| panic!("expected Msg::ProjectScanned, got: {:?}", tags(&msgs)));

    assert_eq!(files, 2, "both files discovered");
    assert_eq!(modules, 1, "only the valid file parses into a module");
    assert_eq!(parse_errs, 1, "one parse error reported");
}

#[tokio::test]
async fn scan_project_missing_dir_emits_status_error() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    command::execute(
        Command::ScanProject,
        "/definitely/not/a/real/path/xyz".into(),
        tx,
    )
    .await;

    let msgs = drain(&mut rx).await;
    assert!(
        msgs.iter().any(|m| matches!(m, Msg::StatusError(_))),
        "expected a StatusError, got: {:?}",
        tags(&msgs)
    );
}

// ── RunAnalyses ─────────────────────────────────────────────────────

#[tokio::test]
async fn run_analyses_emits_lint_and_deps() {
    let project = TempProject::new("run-analyses");
    project.write("Main.elm", GOOD_ELM);

    let (tx, mut rx) = mpsc::unbounded_channel();
    command::execute(Command::RunAnalyses, project.src_dir(), tx).await;

    let msgs = drain(&mut rx).await;
    let has_lint = msgs.iter().any(|m| matches!(m, Msg::LintComplete(_)));
    let has_deps = msgs.iter().any(|m| matches!(m, Msg::DepsComplete { .. }));

    assert!(has_lint, "expected LintComplete, got: {:?}", tags(&msgs));
    assert!(has_deps, "expected DepsComplete, got: {:?}", tags(&msgs));
}

// ── ApplyFix ────────────────────────────────────────────────────────

#[tokio::test]
async fn apply_fix_writes_when_result_parses() {
    let project = TempProject::new("apply-ok");
    let target = project.path_in_src("Target.elm");
    project.write("Target.elm", "module Target exposing (..)\n\nx = 0\n");

    let new_source = "module Target exposing (..)\n\nx = 42\n".to_string();
    let (tx, mut rx) = mpsc::unbounded_channel();
    command::execute(
        Command::ApplyFix(target.clone(), new_source.clone()),
        project.src_dir(),
        tx,
    )
    .await;

    let msgs = drain(&mut rx).await;
    assert!(
        !msgs.iter().any(|m| matches!(m, Msg::StatusError(_))),
        "valid fix should not emit StatusError, got: {:?}",
        tags(&msgs)
    );

    let written = std::fs::read_to_string(&target).expect("file should exist");
    assert_eq!(written, new_source, "file should contain the fixed source");
}

#[tokio::test]
async fn apply_fix_rejects_invalid_elm_without_writing() {
    let project = TempProject::new("apply-invalid");
    let target = project.path_in_src("Target.elm");
    let original = "module Target exposing (..)\n\nx = 0\n";
    project.write("Target.elm", original);

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
    assert!(
        msgs.iter().any(|m| matches!(m, Msg::StatusError(_))),
        "invalid fix should emit StatusError, got: {:?}",
        tags(&msgs)
    );

    let on_disk = std::fs::read_to_string(&target).expect("file should still exist");
    assert_eq!(
        on_disk, original,
        "file must be untouched when fix produces invalid Elm"
    );
}

// ── ExportLintJson ──────────────────────────────────────────────────

#[tokio::test]
async fn export_lint_json_writes_file_and_status_info() {
    use elm_ast::span::{Position, Span};
    use elm_lint::rule::{LintError, Severity};

    let project = TempProject::new("export-json");

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
    assert!(
        msgs.iter().any(|m| matches!(m, Msg::StatusInfo(_))),
        "expected StatusInfo on successful export, got: {:?}",
        tags(&msgs)
    );

    // File resolves to parent-of-src_dir / elm-assist-lint.json.
    let out_path = project.root.join("elm-assist-lint.json");
    let json = std::fs::read_to_string(&out_path).expect("json file should exist");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json must be valid");
    let arr = parsed.as_array().expect("root must be array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["rule"], "NoDebug");
    assert_eq!(arr[0]["severity"], "warning");
    assert_eq!(arr[0]["message"], "debug call found");
    assert_eq!(arr[0]["file"], "src/A.elm");
    assert_eq!(arr[0]["line"], 1);
    assert_eq!(arr[0]["fixable"], false);
}

// ── Batch ───────────────────────────────────────────────────────────

#[tokio::test]
async fn batch_runs_sub_commands_sequentially() {
    let project = TempProject::new("batch");
    let a = project.path_in_src("A.elm");
    let b = project.path_in_src("B.elm");
    project.write("A.elm", "module A exposing (..)\n\nx = 0\n");
    project.write("B.elm", "module B exposing (..)\n\ny = 0\n");

    let a_new = "module A exposing (..)\n\nx = 1\n".to_string();
    let b_new = "module B exposing (..)\n\ny = 2\n".to_string();

    let batch = Command::Batch(vec![
        Command::ApplyFix(a.clone(), a_new.clone()),
        Command::ApplyFix(b.clone(), b_new.clone()),
    ]);

    let (tx, mut rx) = mpsc::unbounded_channel();
    command::execute(batch, project.src_dir(), tx).await;
    let _ = drain(&mut rx).await;

    assert_eq!(std::fs::read_to_string(&a).unwrap(), a_new);
    assert_eq!(std::fs::read_to_string(&b).unwrap(), b_new);
}
