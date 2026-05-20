use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use tokio::sync::Mutex;
use tower::Service;
use tower_lsp::LspService;
use tower_lsp::jsonrpc::{Request, Response};
use tower_lsp::lsp_types::*;

use elm_assist_lsp::backend::Backend;
use test_better::ErrorKind;
use test_better::prelude::*;

fn fail(msg: impl Into<String>) -> TestError {
    TestError::new(ErrorKind::Assertion).with_message(msg.into())
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Messages collected from the server's ClientSocket.
type Messages = Arc<Mutex<Vec<serde_json::Value>>>;

/// Create an LspService, spawn a socket drainer, drive the initialize
/// handshake, and return everything ready for use.
async fn init_service() -> Result<(LspService<Backend>, Messages, InitializeResult), TestError> {
    let (mut service, socket) = LspService::new(Backend::new);

    // Split the socket: read server-to-client messages from the stream,
    // and send responses back through the sink for server requests
    // (like client/registerCapability) that expect a reply.
    let messages: Messages = Arc::new(Mutex::new(Vec::new()));
    let msgs_clone = Arc::clone(&messages);
    tokio::spawn(async move {
        let (mut requests, mut responses) = socket.split();
        while let Some(msg) = requests.next().await {
            // If serialization ever fails, drop the message rather than panic.
            let Ok(json) = serde_json::to_value(&msg) else {
                continue;
            };

            // If the message has an id, it's a server-to-client request that
            // expects a response. Send back an OK result.
            if let Some(id) = msg.id() {
                let resp = Response::from_ok(id.clone(), serde_json::Value::Null);
                let _ = responses.send(resp).await;
            }

            msgs_clone.lock().await.push(json);
        }
    });

    let init_params = InitializeParams {
        root_uri: Some(Url::parse("file:///tmp/test-project").or_fail_with("parse project URI")?),
        capabilities: ClientCapabilities::default(),
        ..Default::default()
    };

    let req = Request::build("initialize")
        .params(
            serde_json::to_value(init_params)
                .map_err(|e| fail(format!("serialize init params: {e}")))?,
        )
        .id(1)
        .finish();

    let resp = service
        .call(req)
        .await
        .map_err(|e| fail(format!("initialize call failed: {e}")))?;
    let init_result: InitializeResult = serde_json::from_value(response_result(resp)?)
        .map_err(|e| fail(format!("valid InitializeResult: {e}")))?;

    // Send initialized notification.
    let notif = Request::build("initialized")
        .params(
            serde_json::to_value(InitializedParams {})
                .map_err(|e| fail(format!("serialize initialized params: {e}")))?,
        )
        .finish();
    let _ = service.call(notif).await;

    // Let the initialized handler complete (it registers watchers + publishes initial diagnostics).
    tokio::time::sleep(Duration::from_millis(100)).await;

    Ok((service, messages, init_result))
}

/// Extract the result field from a JSON-RPC response.
fn response_result(resp: Option<Response>) -> Result<serde_json::Value, TestError> {
    let resp = resp.or_fail_with("expected a response")?;
    let json = serde_json::to_value(resp).map_err(|e| fail(format!("serialize response: {e}")))?;
    Ok(json["result"].clone())
}

/// Send a didOpen notification.
async fn did_open(
    service: &mut LspService<Backend>,
    uri: &Url,
    text: &str,
) -> Result<(), TestError> {
    let params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "elm".into(),
            version: 1,
            text: text.into(),
        },
    };
    let req = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(params)
                .map_err(|e| fail(format!("serialize didOpen params: {e}")))?,
        )
        .finish();
    let _ = service.call(req).await;
    Ok(())
}

/// Send a didChange notification.
async fn did_change(
    service: &mut LspService<Backend>,
    uri: &Url,
    text: &str,
    version: i32,
) -> Result<(), TestError> {
    let params = DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier {
            uri: uri.clone(),
            version,
        },
        content_changes: vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: text.into(),
        }],
    };
    let req = Request::build("textDocument/didChange")
        .params(
            serde_json::to_value(params)
                .map_err(|e| fail(format!("serialize didChange params: {e}")))?,
        )
        .finish();
    let _ = service.call(req).await;
    Ok(())
}

/// Wait for diagnostics and collect publishDiagnostics params for a given URI.
async fn wait_for_diagnostics(
    messages: &Messages,
    uri: &Url,
    timeout_ms: u64,
) -> Vec<PublishDiagnosticsParams> {
    // Wait for server to process (debounce is 150ms in did_change).
    tokio::time::sleep(Duration::from_millis(timeout_ms)).await;

    let msgs = messages.lock().await;
    msgs.iter()
        .filter(|m| m["method"] == "textDocument/publishDiagnostics")
        .filter_map(|m| {
            serde_json::from_value::<PublishDiagnosticsParams>(m["params"].clone()).ok()
        })
        .filter(|p| p.uri == *uri)
        .collect()
}

/// Clear collected messages.
async fn clear_messages(messages: &Messages) {
    messages.lock().await.clear();
}

/// Send a shutdown request.
async fn shutdown(service: &mut LspService<Backend>) {
    let req = Request::build("shutdown").id(99).finish();
    let _ = service.call(req).await;
}

// ── Tests ───────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn initialize_returns_capabilities() -> TestResult {
    let (mut service, _, result) = init_service().await?;

    check!(result.capabilities.text_document_sync).satisfies(eq(Some(
        TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL),
    )))?;
    check!(result.capabilities.code_action_provider)
        .satisfies(eq(Some(CodeActionProviderCapability::Simple(true))))?;
    check!(result.capabilities.hover_provider)
        .satisfies(eq(Some(HoverProviderCapability::Simple(true))))?;

    let info = result
        .server_info
        .ok_or_else(|| fail("expected server_info"))?;
    check!(info.name.as_str()).satisfies(eq("elm-assist-lsp"))?;

    shutdown(&mut service).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn did_open_publishes_diagnostics() -> TestResult {
    let (mut service, messages, _) = init_service().await?;

    let uri = Url::parse("file:///tmp/test-project/src/Test.elm").or_fail_with("parse URI")?;
    let source = "module Test exposing (..)\n\nimport Html\n\nx = 1\n";

    clear_messages(&messages).await;
    did_open(&mut service, &uri, source).await?;

    let diags = wait_for_diagnostics(&messages, &uri, 300).await;
    check!(!diags.is_empty())
        .satisfies(is_true())
        .context("expected diagnostics for opened file")?;

    let all_diags: Vec<_> = diags.iter().flat_map(|d| &d.diagnostics).collect();
    let has_unused_import = all_diags
        .iter()
        .any(|d| d.code == Some(NumberOrString::String("NoUnusedImports".into())));
    check!(has_unused_import)
        .satisfies(is_true())
        .context("expected NoUnusedImports diagnostic")?;

    shutdown(&mut service).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn did_change_updates_diagnostics() -> TestResult {
    let (mut service, messages, _) = init_service().await?;

    let uri = Url::parse("file:///tmp/test-project/src/Test.elm").or_fail_with("parse URI")?;

    // Open with an unused import.
    did_open(
        &mut service,
        &uri,
        "module Test exposing (..)\n\nimport Html\n\nx = 1\n",
    )
    .await?;
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Clear and change to remove the unused import.
    clear_messages(&messages).await;
    did_change(
        &mut service,
        &uri,
        "module Test exposing (..)\n\nx = 1\n",
        2,
    )
    .await?;

    let diags = wait_for_diagnostics(&messages, &uri, 500).await;

    // The latest diagnostics should not contain NoUnusedImports.
    if let Some(last) = diags.last() {
        let has_unused_import = last
            .diagnostics
            .iter()
            .any(|d| d.code == Some(NumberOrString::String("NoUnusedImports".into())));
        check!(has_unused_import)
            .satisfies(is_false())
            .context("NoUnusedImports should be gone after removing import")?;
    }

    shutdown(&mut service).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn did_close_clears_diagnostics() -> TestResult {
    let (mut service, messages, _) = init_service().await?;

    let uri = Url::parse("file:///tmp/test-project/src/Test.elm").or_fail_with("parse URI")?;

    did_open(
        &mut service,
        &uri,
        "module Test exposing (..)\n\nimport Html\n\nx = 1\n",
    )
    .await?;
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Close the document.
    clear_messages(&messages).await;
    let params = DidCloseTextDocumentParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
    };
    let req = Request::build("textDocument/didClose")
        .params(
            serde_json::to_value(params)
                .map_err(|e| fail(format!("serialize didClose params: {e}")))?,
        )
        .finish();
    let _ = service.call(req).await;

    let diags = wait_for_diagnostics(&messages, &uri, 300).await;
    if let Some(last) = diags.last() {
        check!(last.diagnostics.is_empty())
            .satisfies(is_true())
            .context("diagnostics should be cleared on close")?;
    }

    shutdown(&mut service).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hover_on_diagnostic_returns_rule_info() -> TestResult {
    let (mut service, _messages, _) = init_service().await?;

    let uri = Url::parse("file:///tmp/test-project/src/Test.elm").or_fail_with("parse URI")?;
    let source = "module Test exposing (..)\n\nx = Debug.log \"hi\" 1\n";

    did_open(&mut service, &uri, source).await?;
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Hover over "Debug.log" (line 2, around col 6 in 0-based).
    let hover_params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 2,
                character: 6,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    };

    let req = Request::build("textDocument/hover")
        .params(
            serde_json::to_value(hover_params)
                .map_err(|e| fail(format!("serialize hover params: {e}")))?,
        )
        .id(10)
        .finish();

    let resp = service
        .call(req)
        .await
        .map_err(|e| fail(format!("call failed: {e}")))?;
    let result = response_result(resp)?;

    check!(!result.is_null())
        .satisfies(is_true())
        .context("hover should return a result")?;

    let hover: Hover =
        serde_json::from_value(result).map_err(|e| fail(format!("valid Hover: {e}")))?;
    match hover.contents {
        HoverContents::Markup(markup) => {
            check!(markup.value.as_str())
                .satisfies(contains_str("NoDebug"))
                .context(format!(
                    "hover should mention rule name, got: {}",
                    markup.value
                ))?;
        }
        other => return Err(fail(format!("expected Markup hover, got: {other:?}"))),
    }

    shutdown(&mut service).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hover_outside_diagnostic_returns_null() -> TestResult {
    let (mut service, _, _) = init_service().await?;

    let uri = Url::parse("file:///tmp/test-project/src/Test.elm").or_fail_with("parse URI")?;
    let source = "module Test exposing (x)\n\n\n{-| A value. -}\nx : Int\nx =\n    1\n";

    did_open(&mut service, &uri, source).await?;
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Hover on the value body (line 6, "    1") — should have no diagnostic.
    let hover_params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 6,
                character: 4,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    };

    let req = Request::build("textDocument/hover")
        .params(
            serde_json::to_value(hover_params)
                .map_err(|e| fail(format!("serialize hover params: {e}")))?,
        )
        .id(11)
        .finish();

    let resp = service
        .call(req)
        .await
        .map_err(|e| fail(format!("call failed: {e}")))?;
    let result = response_result(resp)?;
    check!(result.is_null())
        .satisfies(is_true())
        .context("hover outside diagnostic should be null")?;

    shutdown(&mut service).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_action_returns_fix() -> TestResult {
    let (mut service, messages, _) = init_service().await?;

    let uri = Url::parse("file:///tmp/test-project/src/Test.elm").or_fail_with("parse URI")?;
    // NoUnusedImports has a fix (removing the import line).
    let source = "module Test exposing (..)\n\nimport Html\n\nx = 1\n";

    did_open(&mut service, &uri, source).await?;
    let diags = wait_for_diagnostics(&messages, &uri, 300).await;

    let all_diags: Vec<_> = diags.iter().flat_map(|d| d.diagnostics.clone()).collect();
    let target_diag = all_diags
        .iter()
        .find(|d| d.code == Some(NumberOrString::String("NoUnusedImports".into())))
        .cloned();

    check!(target_diag.is_some())
        .satisfies(is_true())
        .context(format!(
            "expected NoUnusedImports diagnostic, got: {:?}",
            all_diags.iter().map(|d| &d.code).collect::<Vec<_>>()
        ))?;

    if let Some(diag) = &target_diag {
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: diag.range,
            context: CodeActionContext {
                diagnostics: vec![diag.clone()],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        let req = Request::build("textDocument/codeAction")
            .params(
                serde_json::to_value(params)
                    .map_err(|e| fail(format!("serialize codeAction params: {e}")))?,
            )
            .id(20)
            .finish();

        let resp = service
            .call(req)
            .await
            .map_err(|e| fail(format!("call failed: {e}")))?;
        let result = response_result(resp)?;

        check!(!result.is_null())
            .satisfies(is_true())
            .context(format!(
                "should return code actions for range {:?}, diag code: {:?}",
                diag.range, diag.code
            ))?;
        let actions: Vec<CodeActionOrCommand> =
            serde_json::from_value(result).map_err(|e| fail(format!("valid code actions: {e}")))?;
        check!(!actions.is_empty())
            .satisfies(is_true())
            .context("should have at least one code action")?;

        if let CodeActionOrCommand::CodeAction(action) = &actions[0] {
            check!(action.kind.clone()).satisfies(eq(Some(CodeActionKind::QUICKFIX)))?;
            check!(action.edit.is_some())
                .satisfies(is_true())
                .context("action should have a workspace edit")?;
        } else {
            return Err(fail("expected CodeAction, got Command"));
        }
    } else {
        return Err(fail("expected NoDebug diagnostic"));
    }

    shutdown(&mut service).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_action_on_clean_range_returns_none() -> TestResult {
    let (mut service, _, _) = init_service().await?;

    let uri = Url::parse("file:///tmp/test-project/src/Test.elm").or_fail_with("parse URI")?;
    let source = "module Test exposing (x)\n\n\n{-| A value. -}\nx : Int\nx =\n    1\n";

    did_open(&mut service, &uri, source).await?;
    tokio::time::sleep(Duration::from_millis(300)).await;

    let params = CodeActionParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 5,
            },
        },
        context: CodeActionContext {
            diagnostics: vec![],
            only: None,
            trigger_kind: None,
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let req = Request::build("textDocument/codeAction")
        .params(
            serde_json::to_value(params)
                .map_err(|e| fail(format!("serialize codeAction params: {e}")))?,
        )
        .id(21)
        .finish();

    let resp = service
        .call(req)
        .await
        .map_err(|e| fail(format!("call failed: {e}")))?;
    let result = response_result(resp)?;
    check!(result.is_null())
        .satisfies(is_true())
        .context("no code actions expected on clean range")?;

    shutdown(&mut service).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn parse_error_shows_as_diagnostic() -> TestResult {
    let (mut service, messages, _) = init_service().await?;

    let uri = Url::parse("file:///tmp/test-project/src/Test.elm").or_fail_with("parse URI")?;
    let source = "module Test exposing (..)\n\nx = {{{ invalid\n";

    clear_messages(&messages).await;
    did_open(&mut service, &uri, source).await?;

    let diags = wait_for_diagnostics(&messages, &uri, 300).await;
    let all_diags: Vec<_> = diags.iter().flat_map(|d| &d.diagnostics).collect();

    let has_parse_error = all_diags
        .iter()
        .any(|d| d.code == Some(NumberOrString::String("parse-error".into())));
    check!(has_parse_error)
        .satisfies(is_true())
        .context("expected parse-error diagnostic")?;

    let parse_errors: Vec<_> = all_diags
        .iter()
        .filter(|d| d.code == Some(NumberOrString::String("parse-error".into())))
        .collect();
    for pe in &parse_errors {
        check!(pe.severity).satisfies(eq(Some(DiagnosticSeverity::ERROR)))?;
    }

    shutdown(&mut service).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_succeeds() -> TestResult {
    let (mut service, _, _) = init_service().await?;

    let req = Request::build("shutdown").id(99).finish();
    let resp = service
        .call(req)
        .await
        .map_err(|e| fail(format!("call failed: {e}")))?;
    let result = response_result(resp)?;
    check!(result.is_null())
        .satisfies(is_true())
        .context("shutdown should return null")?;
    Ok(())
}
