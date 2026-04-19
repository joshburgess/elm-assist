use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::analysis;
use crate::convert;
use crate::state::{self, ServerState};

pub struct Backend {
    client: Client,
    state: Arc<RwLock<ServerState>>,
    pending: Arc<Mutex<HashMap<Url, JoinHandle<()>>>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Backend {
            client,
            state: Arc::new(RwLock::new(ServerState::default())),
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Collect all diagnostics (parse errors + lint errors) for a document.
    fn collect_diagnostics(state: &ServerState, uri: &Url) -> Vec<Diagnostic> {
        let Some(doc) = state.documents.get(uri) else {
            return Vec::new();
        };
        let mut diagnostics = convert::parse_errors_to_diagnostics(&doc.parse_errors);
        diagnostics.extend(convert::lint_errors_to_diagnostics(&doc.lint_errors));
        diagnostics
    }

    /// Lint a document, store its errors, and publish diagnostics.
    async fn lint_and_publish(&self, uri: &Url) {
        let mut state = self.state.write().await;
        let errors = analysis::lint_document(&state, uri);

        if let Some(doc) = state.documents.get_mut(uri) {
            doc.lint_errors = errors;
        }

        let diagnostics = Self::collect_diagnostics(&state, uri);
        let version = state.documents.get(uri).map(|d| d.version).unwrap_or(0);

        drop(state);

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, Some(version))
            .await;
    }

    /// Lint all documents and publish diagnostics for each.
    async fn lint_all_and_publish(&self) {
        let mut state = self.state.write().await;
        let all_errors = analysis::lint_all_open(&state);

        let mut to_publish = Vec::new();
        for (uri, errors) in all_errors {
            if let Some(doc) = state.documents.get_mut(&uri) {
                doc.lint_errors = errors;
            }
            let diagnostics = Self::collect_diagnostics(&state, &uri);
            let version = state.documents.get(&uri).map(|d| d.version).unwrap_or(0);
            to_publish.push((uri, diagnostics, version));
        }

        drop(state);

        for (uri, diagnostics, version) in to_publish {
            self.client
                .publish_diagnostics(uri, diagnostics, Some(version))
                .await;
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Determine workspace root.
        let root = params
            .root_uri
            .as_ref()
            .and_then(|uri| uri.to_file_path().ok())
            .or_else(|| {
                #[allow(deprecated)]
                params.root_path.as_ref().map(std::path::PathBuf::from)
            })
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        // Initialize server state with project scan.
        let new_state = ServerState::new(root);
        let mut state = self.state.write().await;
        *state = new_state;
        drop(state);

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "elm-assist-lsp".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "elm-assist-lsp initialized")
            .await;

        // Register file watchers for .elm files and config.
        let watchers = vec![
            FileSystemWatcher {
                glob_pattern: GlobPattern::String("**/*.elm".into()),
                kind: None,
            },
            FileSystemWatcher {
                glob_pattern: GlobPattern::String("**/elm-assist.toml".into()),
                kind: None,
            },
        ];

        let registration = Registration {
            id: "watch-files".into(),
            method: "workspace/didChangeWatchedFiles".into(),
            register_options: Some(
                serde_json::to_value(DidChangeWatchedFilesRegistrationOptions { watchers })
                    .unwrap(),
            ),
        };

        if let Err(e) = self.client.register_capability(vec![registration]).await {
            self.client
                .log_message(
                    MessageType::WARNING,
                    format!("Failed to register file watchers: {e}"),
                )
                .await;
        }

        // Publish initial diagnostics for all scanned files.
        self.lint_all_and_publish().await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        let version = params.text_document.version;

        let needs_rebuild = {
            let mut state = self.state.write().await;
            state.update_document(&uri, text, version)
        };

        if needs_rebuild {
            {
                let mut state = self.state.write().await;
                state.rebuild_project_context();
            }
            self.lint_all_and_publish().await;
        } else {
            self.lint_and_publish(&uri).await;
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;

        // Full sync: take the last content change (should be exactly one).
        let Some(change) = params.content_changes.into_iter().last() else {
            return;
        };
        let text = change.text;

        // Cancel any pending analysis for this URI.
        let mut pending = self.pending.lock().await;
        if let Some(handle) = pending.remove(&uri) {
            handle.abort();
        }

        // Spawn debounced analysis.
        let state_arc = Arc::clone(&self.state);
        let client = self.client.clone();
        let uri_clone = uri.clone();

        let handle = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;

            let needs_rebuild = {
                let mut s = state_arc.write().await;
                s.update_document(&uri_clone, text, version)
            };

            if needs_rebuild {
                let mut s = state_arc.write().await;
                s.rebuild_project_context();

                let all_errors = analysis::lint_all_open(&s);
                let mut to_publish = Vec::new();
                for (u, errors) in all_errors {
                    if let Some(doc) = s.documents.get_mut(&u) {
                        doc.lint_errors = errors;
                    }
                    let diagnostics = Backend::collect_diagnostics(&s, &u);
                    let v = s.documents.get(&u).map(|d| d.version).unwrap_or(0);
                    to_publish.push((u, diagnostics, v));
                }
                drop(s);

                for (u, diagnostics, v) in to_publish {
                    client.publish_diagnostics(u, diagnostics, Some(v)).await;
                }
            } else {
                let mut s = state_arc.write().await;
                let errors = analysis::lint_document(&s, &uri_clone);
                if let Some(doc) = s.documents.get_mut(&uri_clone) {
                    doc.lint_errors = errors;
                }
                let diagnostics = Backend::collect_diagnostics(&s, &uri_clone);
                let v = s.documents.get(&uri_clone).map(|d| d.version).unwrap_or(0);
                drop(s);

                client
                    .publish_diagnostics(uri_clone, diagnostics, Some(v))
                    .await;
            }
        });

        pending.insert(uri, handle);
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;

        if let Some(text) = params.text {
            let needs_rebuild = {
                let mut state = self.state.write().await;
                state.update_document(&uri, text, 0)
            };
            if needs_rebuild {
                {
                    let mut state = self.state.write().await;
                    state.rebuild_project_context();
                }
                self.lint_all_and_publish().await;
            } else {
                self.lint_and_publish(&uri).await;
            }
        } else {
            self.lint_and_publish(&uri).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;

        self.client
            .publish_diagnostics(uri.clone(), Vec::new(), None)
            .await;

        let mut pending = self.pending.lock().await;
        if let Some(handle) = pending.remove(&uri) {
            handle.abort();
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let state = self.state.read().await;
        let Some(doc) = state.documents.get(uri) else {
            return Ok(None);
        };

        // Find a lint error whose range covers the hover position.
        for error in &doc.lint_errors {
            let error_range = convert::span_to_range(&error.span);
            if !position_in_range(&pos, &error_range) {
                continue;
            }

            // Look up rule description.
            if let Some(info) = state.rule_descriptions.get(error.rule) {
                let fixable_note = if info.fixable {
                    "auto-fixable"
                } else {
                    "not auto-fixable"
                };
                let content = format!(
                    "**{}**\n\n{}\n\n---\n*elm-assist rule — {}*",
                    error.rule, info.description, fixable_note
                );
                return Ok(Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: content,
                    }),
                    range: Some(error_range),
                }));
            }
        }

        Ok(None)
    }

    async fn did_change_configuration(&self, _params: DidChangeConfigurationParams) {
        // Reload config from disk and re-lint everything.
        {
            let mut state = self.state.write().await;
            state.reload_config();
        }
        self.lint_all_and_publish().await;

        self.client
            .log_message(MessageType::INFO, "elm-assist-lsp: configuration reloaded")
            .await;
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        let mut needs_config_reload = false;
        let mut needs_file_rescan = false;

        for change in &params.changes {
            let path = change.uri.path();
            if path.ends_with("elm-assist.toml") {
                needs_config_reload = true;
            } else if path.ends_with(".elm") {
                needs_file_rescan = true;
            }
        }

        if needs_config_reload {
            let mut state = self.state.write().await;
            state.reload_config();
        }

        if needs_file_rescan {
            // Re-read changed/created files from disk.
            let mut state = self.state.write().await;
            for change in &params.changes {
                if !change.uri.path().ends_with(".elm") {
                    continue;
                }
                match change.typ {
                    FileChangeType::CREATED | FileChangeType::CHANGED => {
                        let file_path = state::uri_to_file_path(&change.uri);
                        if let Ok(source) = std::fs::read_to_string(&file_path) {
                            state.update_document(&change.uri, source, 0);
                        }
                    }
                    FileChangeType::DELETED => {
                        state.documents.remove(&change.uri);
                    }
                    _ => {}
                }
            }
            state.rebuild_project_context();
        }

        if needs_config_reload || needs_file_rescan {
            self.lint_all_and_publish().await;
        }
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = &params.text_document.uri;

        let source = {
            let state = self.state.read().await;
            let Some(doc) = state.documents.get(uri) else {
                return Ok(None);
            };
            doc.source.clone()
        };

        let module = match elm_ast::parse(&source) {
            Ok(m) => m,
            Err(_) => {
                self.client
                    .log_message(
                        MessageType::INFO,
                        "elm-assist-lsp: formatting skipped (parse error)",
                    )
                    .await;
                return Ok(None);
            }
        };

        let formatted = elm_ast::pretty_print(&module);
        if formatted == source {
            return Ok(Some(Vec::new()));
        }

        Ok(Some(vec![TextEdit {
            range: full_document_range(&source),
            new_text: formatted,
        }]))
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = &params.text_document.uri;
        let range = params.range;

        let state = self.state.read().await;
        let Some(doc) = state.documents.get(uri) else {
            return Ok(None);
        };

        let mut actions = Vec::new();
        for error in &doc.lint_errors {
            let error_range = convert::span_to_range(&error.span);
            if !ranges_overlap(&error_range, &range) {
                continue;
            }

            if let Some(action) = convert::fix_to_code_action(uri, error) {
                actions.push(CodeActionOrCommand::CodeAction(action));
            }
        }

        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }
}

/// Check if a position is within a range (inclusive).
fn position_in_range(pos: &Position, range: &Range) -> bool {
    if pos.line < range.start.line || pos.line > range.end.line {
        return false;
    }
    if pos.line == range.start.line && pos.character < range.start.character {
        return false;
    }
    if pos.line == range.end.line && pos.character > range.end.character {
        return false;
    }
    true
}

/// Check if two ranges overlap.
fn ranges_overlap(a: &Range, b: &Range) -> bool {
    a.start <= b.end && b.start <= a.end
}

/// Compute a Range covering an entire document, in UTF-16 code units per
/// the LSP spec.
fn full_document_range(source: &str) -> Range {
    let mut line: u32 = 0;
    let mut last_line_utf16: u32 = 0;
    for ch in source.chars() {
        if ch == '\n' {
            line += 1;
            last_line_utf16 = 0;
        } else {
            last_line_utf16 += ch.len_utf16() as u32;
        }
    }
    Range {
        start: Position::new(0, 0),
        end: Position::new(line, last_line_utf16),
    }
}
