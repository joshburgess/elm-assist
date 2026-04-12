//! Async command execution — runs tool operations on background threads
//! and sends results back as Msgs.

use std::fs;
use std::path::Path;

use tokio::sync::mpsc;

use elm_deps::graph;
use elm_lint::config::Config;
use elm_lint::pipeline;
use elm_lint::rules;

use crate::app::{Command, Msg};

/// Execute a command, sending result messages into the channel.
pub fn execute(
    cmd: Command,
    src_dir: String,
    tx: mpsc::UnboundedSender<Msg>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
    Box::pin(async move {
        execute_inner(cmd, src_dir, tx).await;
    })
}

async fn execute_inner(cmd: Command, src_dir: String, tx: mpsc::UnboundedSender<Msg>) {
    match cmd {
        Command::None => {}
        Command::LoadPreview(file_path, line) => {
            let tx2 = tx.clone();
            tokio::task::spawn_blocking(move || {
                if let Ok(source) = fs::read_to_string(&file_path) {
                    let _ = tx2.send(Msg::PreviewLoaded {
                        file_path,
                        line,
                        source: std::sync::Arc::new(source),
                    });
                }
            })
            .await
            .ok();
        }
        Command::ExportLintJson(errors) => {
            let tx2 = tx.clone();
            let project_dir = src_dir.clone();
            tokio::task::spawn_blocking(move || {
                let entries: Vec<serde_json::Value> = errors
                    .as_ref()
                    .iter()
                    .map(|(path, err)| {
                        serde_json::json!({
                            "file": path,
                            "line": err.span.start.line,
                            "column": err.span.start.column,
                            "endLine": err.span.end.line,
                            "endColumn": err.span.end.column,
                            "rule": err.rule,
                            "severity": match err.severity {
                                elm_lint::rule::Severity::Error => "error",
                                elm_lint::rule::Severity::Warning => "warning",
                            },
                            "message": err.message,
                            "fixable": err.fix.is_some(),
                        })
                    })
                    .collect();

                // Resolve output path to the project root (parent of src dir, or
                // src dir itself if it has no parent).
                let out_path = {
                    let src = Path::new(&project_dir);
                    let base = src.parent().unwrap_or(src);
                    base.join("elm-assist-lint.json")
                };

                let json = match serde_json::to_string_pretty(&entries) {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = tx2.send(Msg::StatusError(format!(
                            "Export failed: could not serialize diagnostics: {e}"
                        )));
                        return;
                    }
                };
                match fs::write(&out_path, &json) {
                    Ok(()) => {
                        let abs = out_path.canonicalize().unwrap_or_else(|_| out_path.clone());
                        let _ = tx2.send(Msg::StatusInfo(format!(
                            "Exported {} diagnostics to {}",
                            entries.len(),
                            abs.display()
                        )));
                    }
                    Err(e) => {
                        let _ = tx2.send(Msg::StatusError(format!(
                            "Export failed: {} ({e})",
                            out_path.display()
                        )));
                    }
                }
            })
            .await
            .ok();
        }
        Command::ClearStatusAfter(msg_gen) => {
            let tx2 = tx.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                let _ = tx2.send(Msg::ClearStatus(msg_gen));
            });
        }
        Command::Batch(cmds) => {
            // Sequential: each sub-command must complete before the next runs.
            // This is required so that e.g. ApplyFix writes land before a
            // subsequent RunLint reads them.
            for c in cmds {
                execute(c, src_dir.clone(), tx.clone()).await;
            }
        }
        Command::ApplyFix(file_path, fixed_source) => {
            let tx2 = tx.clone();
            tokio::task::spawn_blocking(move || {
                // Validate the fix produces valid Elm before writing.
                if elm_ast::parse(&fixed_source).is_err() {
                    let _ = tx2.send(Msg::StatusError(format!(
                        "Fix rejected: produced invalid Elm in {file_path}"
                    )));
                    return;
                }
                if let Err(e) = fs::write(&file_path, &fixed_source) {
                    let _ = tx2.send(Msg::StatusError(format!(
                        "Failed to write {file_path}: {e}"
                    )));
                }
            })
            .await
            .ok();
        }
        Command::ScanProject => {
            let dir = src_dir.clone();
            let tx2 = tx.clone();
            tokio::task::spawn_blocking(move || {
                if !Path::new(&dir).exists() {
                    let _ = tx2.send(Msg::StatusError(format!(
                        "Source directory '{}' not found",
                        dir
                    )));
                    return;
                }
                let files = pipeline::discover_files(&dir);
                let parsed = pipeline::parse_files(&files);
                let _ = tx2.send(Msg::ProjectScanned {
                    module_count: parsed.files.len(),
                    file_count: files.len(),
                    parse_error_count: parsed.parse_error_count,
                    parse_errors: parsed.parse_errors,
                });
            })
            .await
            .ok();
        }
        Command::RunAnalyses => {
            let dir = src_dir.clone();
            let tx2 = tx.clone();
            tokio::task::spawn_blocking(move || {
                let config = Config::discover().map(|(_, c)| c).unwrap_or_default();
                let mut all_rules = rules::all_rules();
                for rule in &mut all_rules {
                    if let Some(options) = config.rule_options(rule.name()) {
                        let _ = rule.configure(options);
                    }
                }
                let active: Vec<&dyn elm_lint::rule::Rule> = all_rules
                    .iter()
                    .filter(|r| !config.is_rule_disabled(r.name()))
                    .map(|r| r.as_ref())
                    .collect();

                // Single parse pass feeds both lint and deps.
                let analysis = pipeline::run_all(&dir, &active, &config);
                let (internal_graph, _) = graph::build_graph(&analysis.module_data);
                let stats = graph::compute_stats(&internal_graph);

                let _ = tx2.send(Msg::DepsComplete {
                    stats,
                    graph_data: analysis.module_data,
                });
                // Send LintComplete second so the unused-findings derivation
                // lands after the deps state is populated (order is arbitrary,
                // but this keeps the "Running lint..." spinner visible longer).
                let _ = tx2.send(Msg::LintComplete(analysis.lint));
            })
            .await
            .ok();
        }
        Command::RunSearch(query) => {
            let dir = src_dir.clone();
            let tx2 = tx.clone();
            tokio::task::spawn_blocking(move || match run_search(&dir, &query) {
                Ok(results) => {
                    let _ = tx2.send(Msg::SearchComplete(results));
                }
                Err(err) => {
                    let _ = tx2.send(Msg::StatusError(format!("Search error: {err}")));
                    let _ = tx2.send(Msg::SearchComplete(Vec::new()));
                }
            })
            .await
            .ok();
        }
    }
}

fn run_search(dir: &str, query: &str) -> Result<Vec<crate::app::SearchResult>, String> {
    let parsed_query = elm_search::query::parse_query(query)?;

    let files = pipeline::discover_files(dir);
    let mut results = Vec::new();

    for file in &files {
        let source = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if let Ok(module) = elm_ast::parse(&source) {
            let matches = elm_search::search::search(&module, &parsed_query);
            for m in matches {
                results.push(crate::app::SearchResult {
                    file_path: file.display().to_string(),
                    line: m.span.start.line as usize,
                    context: m.context.clone(),
                });
            }
        }
    }

    Ok(results)
}
