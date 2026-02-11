mod common;

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use common::{fixture_path, has_metal_compiler, read_fixture};
use futures::{SinkExt, StreamExt};
use serde_json::json;
use tower::Service;
use tower::ServiceExt;
use tower_lsp::jsonrpc::{Request, Response};
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeConfigurationParams, DidChangeTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, InitializedParams,
    PublishDiagnosticsParams, TextDocumentContentChangeEvent, TextDocumentIdentifier,
    TextDocumentItem, VersionedTextDocumentIdentifier, Url,
};
use tower_lsp::{ClientSocket, LspService};

use metal_analyzer::MetalLanguageServer;

async fn initialize_service_with_params(
    initialize_params: serde_json::Value,
) -> (LspService<MetalLanguageServer>, ClientSocket) {
    let (mut service, socket) = LspService::new(|client| MetalLanguageServer::new(client, false));

    let initialize = Request::build("initialize")
        .params(initialize_params)
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .expect("service ready")
        .call(initialize)
        .await
        .expect("initialize call");
    assert!(init_response.is_some(), "initialize should return a response");

    let initialized = Request::build("initialized")
        .params(serde_json::to_value(InitializedParams {}).expect("serialize initialized params"))
        .finish();
    let initialized_response = service
        .ready()
        .await
        .expect("service ready")
        .call(initialized)
        .await
        .expect("initialized call");
    assert!(
        initialized_response.is_none(),
        "initialized notification should not return a response"
    );

    (service, socket)
}

async fn initialize_service() -> (LspService<MetalLanguageServer>, ClientSocket) {
    initialize_service_with_params(json!({ "capabilities": {} })).await
}

fn temporary_workspace_dir(test_name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after UNIX_EPOCH")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "metal-analyzer-{test_name}-{}-{unique}",
        std::process::id()
    ))
}

async fn send_notification<P: serde::Serialize>(
    service: &mut LspService<MetalLanguageServer>,
    socket: &mut ClientSocket,
    pending_notifications: &mut Vec<Request>,
    method: &'static str,
    params: P,
) {
    let request = Request::build(method)
        .params(serde_json::to_value(params).expect("serialize notification params"))
        .finish();
    let mut call_fut = Box::pin(async {
        service
            .ready()
            .await
            .expect("service ready")
            .call(request)
            .await
            .expect("notification call")
    });

    loop {
        tokio::select! {
            response = &mut call_fut => {
                assert!(response.is_none(), "{method} should be handled as notification");
                break;
            }
            maybe_req = socket.next() => {
                let req = maybe_req.expect("client socket unexpectedly closed while handling notification");
                if let Some(id) = req.id().cloned() {
                    let response = Response::from_ok(id, json!(null));
                    socket
                        .send(response)
                        .await
                        .expect("failed to send synthetic client response");
                } else {
                    pending_notifications.push(req);
                }
            }
        }
    }
}

fn parse_publish_for_uri(req: &Request, uri: &Url) -> Option<PublishDiagnosticsParams> {
    if req.method() != "textDocument/publishDiagnostics" {
        return None;
    }
    let params_value = req.params().cloned()?;
    let params: PublishDiagnosticsParams = serde_json::from_value(params_value).ok()?;
    if params.uri == *uri {
        Some(params)
    } else {
        None
    }
}

async fn next_publish_for_uri(
    socket: &mut ClientSocket,
    pending_notifications: &mut Vec<Request>,
    uri: &Url,
) -> PublishDiagnosticsParams {
    if let Some(idx) = pending_notifications
        .iter()
        .position(|req| parse_publish_for_uri(req, uri).is_some())
    {
        let req = pending_notifications.remove(idx);
        return parse_publish_for_uri(&req, uri).expect("publish request should parse");
    }

    loop {
        let maybe_req = tokio::time::timeout(Duration::from_secs(20), socket.next())
            .await
            .expect("timed out waiting for server notifications");
        let req = maybe_req.expect("client socket unexpectedly closed");
        if let Some(id) = req.id().cloned() {
            let response = Response::from_ok(id, json!(null));
            socket
                .send(response)
                .await
                .expect("failed to send synthetic client response");
            continue;
        }
        if let Some(params) = parse_publish_for_uri(&req, uri) {
            return params;
        }
    }
}

fn has_redefine_warning(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|d| d.message.to_lowercase().contains("redefine"))
}

fn has_owner_context_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|d| {
        d.severity == Some(DiagnosticSeverity::ERROR)
            && (d.message.contains("owner_missing_symbol")
                || d.message.to_lowercase().contains("undeclared"))
    })
}

#[tokio::test]
async fn open_change_save_suppresses_macro_redefinition_noise() {
    let (mut service, mut socket) = initialize_service().await;
    let mut pending_notifications = Vec::new();

    let rel = "matmul/gemv/shaders/gemv_like.metal";
    let uri = Url::from_file_path(fixture_path(rel)).expect("fixture URI");
    let source = read_fixture(rel);
    let fixed_source = source
        .lines()
        .filter(|line| !line.contains("#define MTL_CONST static constant constexpr const"))
        .collect::<Vec<_>>()
        .join("\n");

    send_notification(
        &mut service,
        &mut socket,
        &mut pending_notifications,
        "textDocument/didOpen",
        DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "metal".to_owned(),
                version: 1,
                text: source.clone(),
            },
        },
    )
    .await;
    let open_diags = next_publish_for_uri(&mut socket, &mut pending_notifications, &uri).await;
    assert!(
        !has_redefine_warning(&open_diags.diagnostics),
        "didOpen should suppress macro-redefine noise for this fixture: {:?}",
        open_diags.diagnostics
    );

    send_notification(
        &mut service,
        &mut socket,
        &mut pending_notifications,
        "textDocument/didSave",
        DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            text: None,
        },
    )
    .await;
    let save_diags = next_publish_for_uri(&mut socket, &mut pending_notifications, &uri).await;
    assert!(
        !has_redefine_warning(&save_diags.diagnostics),
        "didSave should preserve macro-redefine suppression semantics: {:?}",
        save_diags.diagnostics
    );

    send_notification(
        &mut service,
        &mut socket,
        &mut pending_notifications,
        "textDocument/didChange",
        DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version: 2,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: fixed_source,
            }],
        },
    )
    .await;
    let change_diags = next_publish_for_uri(&mut socket, &mut pending_notifications, &uri).await;
    assert!(
        !has_redefine_warning(&change_diags.diagnostics),
        "didChange should keep macro-redefine warning suppressed: {:?}",
        change_diags.diagnostics
    );
}

#[tokio::test]
async fn header_open_and_change_use_owner_context_diagnostics() {
    let (mut service, mut socket) = initialize_service().await;
    let mut pending_notifications = Vec::new();

    let owner_rel = "matmul/gemv/shaders/owner_context.metal";
    let owner_uri = Url::from_file_path(fixture_path(owner_rel)).expect("owner URI");
    let owner_source = read_fixture(owner_rel);

    send_notification(
        &mut service,
        &mut socket,
        &mut pending_notifications,
        "textDocument/didOpen",
        DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: owner_uri,
                language_id: "metal".to_owned(),
                version: 1,
                text: owner_source,
            },
        },
    )
    .await;
    let _ = next_publish_for_uri(
        &mut socket,
        &mut pending_notifications,
        &Url::from_file_path(fixture_path(owner_rel)).expect("owner URI"),
    )
    .await;

    let header_rel = "matmul/common/problematic_owner_only.h";
    let header_uri = Url::from_file_path(fixture_path(header_rel)).expect("header URI");
    let header_source = read_fixture(header_rel);

    send_notification(
        &mut service,
        &mut socket,
        &mut pending_notifications,
        "textDocument/didOpen",
        DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: header_uri.clone(),
                language_id: "cpp".to_owned(),
                version: 1,
                text: header_source.clone(),
            },
        },
    )
    .await;
    let open_diags = next_publish_for_uri(&mut socket, &mut pending_notifications, &header_uri).await;
    assert!(
        !has_owner_context_errors(&open_diags.diagnostics),
        "didOpen for header should avoid standalone-owner-context errors: {:?}",
        open_diags.diagnostics
    );

    send_notification(
        &mut service,
        &mut socket,
        &mut pending_notifications,
        "textDocument/didChange",
        DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: header_uri.clone(),
                version: 2,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: format!("{header_source}\n"),
            }],
        },
    )
    .await;
    let change_diags = next_publish_for_uri(&mut socket, &mut pending_notifications, &header_uri).await;
    assert!(
        !has_owner_context_errors(&change_diags.diagnostics),
        "didChange for header should keep owner-context filtering: {:?}",
        change_diags.diagnostics
    );
}

#[tokio::test]
async fn workspace_scope_publishes_diagnostics_for_unopened_files() {
    if !has_metal_compiler() {
        return;
    }

    let workspace_dir = temporary_workspace_dir("workspace-diagnostics");
    std::fs::create_dir_all(&workspace_dir).expect("temporary workspace directory should be created");
    let file_path = workspace_dir.join("broken_workspace_shader.metal");
    let source = r#"
#include <metal_stdlib>
using namespace metal;

kernel void broken_kernel(device float* data [[buffer(0)]], uint tid [[thread_position_in_grid]]) {
    data[tid] = missing_workspace_symbol;
}
"#;
    std::fs::write(&file_path, source).expect("temporary workspace shader should be written");

    let workspace_uri = Url::from_directory_path(&workspace_dir).expect("workspace URI");
    let canonical_file_path = file_path
        .canonicalize()
        .expect("temporary workspace shader path should canonicalize");
    let file_uri = Url::from_file_path(&canonical_file_path).expect("file URI");
    let init_params = json!({
        "capabilities": {},
        "rootUri": workspace_uri.as_str(),
        "workspaceFolders": [
            {
                "uri": workspace_uri.as_str(),
                "name": "workspace"
            }
        ],
        "initializationOptions": {
            "metal-analyzer": {
                "indexing": {
                    "enabled": false
                }
            }
        }
    });
    let (mut service, mut socket) = initialize_service_with_params(init_params).await;
    let mut pending_notifications = Vec::new();

    send_notification(
        &mut service,
        &mut socket,
        &mut pending_notifications,
        "workspace/didChangeConfiguration",
        DidChangeConfigurationParams {
            settings: json!({
                "metal-analyzer": {
                    "diagnostics": {
                        "onType": false,
                        "onSave": false,
                        "scope": "workspace"
                    },
                    "indexing": {
                        "enabled": false
                    },
                    "logging": {
                        "level": "error"
                    }
                }
            }),
        },
    )
    .await;

    let published = next_publish_for_uri(&mut socket, &mut pending_notifications, &file_uri).await;
    assert_eq!(published.uri, file_uri);
    assert!(
        published.version.is_none(),
        "workspace diagnostics for unopened files should not attach a document version"
    );

    std::fs::remove_dir_all(&workspace_dir).expect("temporary workspace directory should be cleaned");
}
