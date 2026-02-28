mod common;

use std::{path::PathBuf, time::Duration};

use common::{fixture_path, has_metal_compiler, position_of, read_fixture};
use futures::{SinkExt, StreamExt};
use metal_analyzer::MetalLanguageServer;
use serde_json::json;
use tower::{Service, ServiceExt};
use tower_lsp::{
    ClientSocket, LspService,
    jsonrpc::{Request, Response},
    lsp_types::{
        DidOpenTextDocumentParams, GotoDefinitionParams, GotoDefinitionResponse, InitializedParams,
        PartialResultParams, TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams, Url,
        WorkDoneProgressParams,
    },
};
use walkdir::WalkDir;

async fn initialize_service() -> (LspService<MetalLanguageServer>, ClientSocket) {
    let (mut service, socket) = LspService::new(|client| MetalLanguageServer::new(client, false));

    let initialize = Request::build("initialize")
        .params(json!({
            "capabilities": {},
            "initializationOptions": {
                "metal-analyzer": {
                    "diagnostics": {
                        "onType": false,
                        "onSave": false
                    },
                    "indexing": {
                        "enable": false
                    },
                    "logging": {
                        "level": "error"
                    }
                }
            }
        }))
        .id(1)
        .finish();
    let init_response = service.ready().await.expect("service ready").call(initialize).await.expect("initialize call");
    assert!(init_response.is_some(), "initialize should return a response");

    let initialized = Request::build("initialized")
        .params(serde_json::to_value(InitializedParams {}).expect("serialize initialized params"))
        .finish();
    let initialized_response =
        service.ready().await.expect("service ready").call(initialized).await.expect("initialized call");
    assert!(initialized_response.is_none(), "initialized notification should not return a response");

    (service, socket)
}

async fn send_notification<P: serde::Serialize>(
    service: &mut LspService<MetalLanguageServer>,
    socket: &mut ClientSocket,
    pending_notifications: &mut Vec<Request>,
    method: &'static str,
    params: P,
) {
    let request =
        Request::build(method).params(serde_json::to_value(params).expect("serialize notification params")).finish();
    let mut call_fut = Box::pin(async {
        service.ready().await.expect("service ready").call(request).await.expect("notification call")
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

async fn send_request_with_client_pump<P: serde::Serialize>(
    service: &mut LspService<MetalLanguageServer>,
    socket: &mut ClientSocket,
    pending_notifications: &mut Vec<Request>,
    method: &'static str,
    params: P,
    id: i64,
) -> (Response, Vec<String>) {
    let request =
        Request::build(method).params(serde_json::to_value(params).expect("serialize request params")).id(id).finish();
    let mut call_fut =
        Box::pin(async { service.ready().await.expect("service ready").call(request).await.expect("request call") });
    let mut server_requests = Vec::new();

    loop {
        tokio::select! {
            maybe_response = &mut call_fut => {
                let response = maybe_response.expect("request should return response");
                return (response, server_requests);
            }
            maybe_req = tokio::time::timeout(Duration::from_secs(20), socket.next()) => {
                let maybe_req = maybe_req
                    .expect("timed out waiting for server message while request is in flight");
                let req = maybe_req.expect("client socket unexpectedly closed while request in flight");
                if let Some(id) = req.id().cloned() {
                    server_requests.push(req.method().to_string());
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

fn first_location(resp: GotoDefinitionResponse) -> tower_lsp::lsp_types::Location {
    match resp {
        GotoDefinitionResponse::Scalar(loc) => loc,
        GotoDefinitionResponse::Array(mut locs) => locs.remove(0),
        GotoDefinitionResponse::Link(mut links) => {
            let link = links.remove(0);
            tower_lsp::lsp_types::Location {
                uri: link.target_uri,
                range: link.target_selection_range,
            }
        },
    }
}

fn external_attention_fixture_paths() -> Option<(PathBuf, PathBuf)> {
    let external_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../external");
    if !external_root.exists() {
        return None;
    }

    let source_path = WalkDir::new(&external_root)
        .into_iter()
        .filter_map(Result::ok)
        .map(|entry| entry.into_path())
        .find(|path| {
            let is_metal_file =
                path.extension().and_then(|extension| extension.to_str()).is_some_and(|extension| extension == "metal");
            let parent_is_attention_dir = path
                .parent()
                .and_then(|parent| parent.file_name())
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "attention");
            is_metal_file && parent_is_attention_dir
        })?;

    let header_path = source_path.ancestors().find_map(|ancestor| {
        let candidate = ancestor.join("generated").join("attention.h");
        candidate.is_file().then_some(candidate)
    })?;

    Some((source_path, header_path))
}

#[tokio::test]
async fn goto_definition_emits_navigation_progress_notifications() {
    if !has_metal_compiler() {
        return;
    }

    let (mut service, mut socket) = initialize_service().await;
    let mut pending_notifications = Vec::new();

    let rel = "matmul/gemv/shaders/gemv_like.metal";
    let uri = Url::from_file_path(fixture_path(rel)).expect("fixture URI");
    let source = read_fixture(rel);
    let position = position_of(&source, "local_template(sum.re)");

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
                text: source,
            },
        },
    )
    .await;

    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.clone(),
            },
            position,
        },
        work_done_progress_params: WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: PartialResultParams {
            partial_result_token: None,
        },
    };

    let (response, server_requests) = send_request_with_client_pump(
        &mut service,
        &mut socket,
        &mut pending_notifications,
        "textDocument/definition",
        params,
        2,
    )
    .await;
    assert!(response.is_ok(), "goto-definition should return successful response, got error: {:?}", response.error());

    let result = response.result().cloned().expect("response result");
    let definition = serde_json::from_value::<Option<GotoDefinitionResponse>>(result)
        .expect("deserialize goto-definition payload")
        .expect("definition result should be available");
    let target = first_location(definition);
    assert_eq!(target.uri.path(), uri.path(), "definition should stay in same file");
    assert_eq!(target.range.start.line, 16, "local_template definition line should match fixture");

    assert!(
        server_requests.iter().any(|method| method == "window/workDoneProgress/create"),
        "navigation should trigger workDoneProgress/create roundtrip"
    );
    assert!(
        pending_notifications.iter().any(|notification| notification.method() == "$/progress"),
        "navigation should emit $/progress notifications"
    );
}

#[tokio::test]
async fn goto_definition_resolves_external_attention_fixture_types_and_fields() {
    if !has_metal_compiler() {
        return;
    }

    let Some((source_path, _header_path)) = external_attention_fixture_paths() else {
        // Optional external fixture may not exist in all environments.
        return;
    };

    let source = std::fs::read_to_string(&source_path).expect("read external attention fixture");
    let uri = Url::from_file_path(&source_path).expect("external fixture URI");
    let position_for = |needle: &str| position_of(&source, needle);

    let (mut service, mut socket) = initialize_service().await;
    let mut pending_notifications = Vec::new();

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

    // 1) `AttnParams` in kernel signature should resolve to generated/attention.h.
    let type_params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.clone(),
            },
            position: position_for("AttnParams* params [[buffer(4)]]"),
        },
        work_done_progress_params: WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: PartialResultParams {
            partial_result_token: None,
        },
    };

    let (type_response, _) = send_request_with_client_pump(
        &mut service,
        &mut socket,
        &mut pending_notifications,
        "textDocument/definition",
        type_params,
        2,
    )
    .await;
    assert!(type_response.is_ok(), "AttnParams goto-definition should succeed, got error: {:?}", type_response.error());
    let type_result = type_response.result().cloned().expect("type result");
    let type_definition = serde_json::from_value::<Option<GotoDefinitionResponse>>(type_result)
        .expect("deserialize type goto-definition payload")
        .expect("AttnParams definition should be available");
    let type_target = first_location(type_definition);
    assert!(
        type_target.uri.path().ends_with("generated/attention.h"),
        "AttnParams should resolve to generated attention header, got {}",
        type_target.uri.path()
    );

    // 2) `AttnMaskParams` in kernel signature should resolve to generated/attention.h.
    let mask_type_params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.clone(),
            },
            position: position_for("AttnMaskParams* mask_params"),
        },
        work_done_progress_params: WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: PartialResultParams {
            partial_result_token: None,
        },
    };

    let (mask_type_response, _) = send_request_with_client_pump(
        &mut service,
        &mut socket,
        &mut pending_notifications,
        "textDocument/definition",
        mask_type_params,
        3,
    )
    .await;
    assert!(
        mask_type_response.is_ok(),
        "AttnMaskParams goto-definition should succeed, got error: {:?}",
        mask_type_response.error()
    );
    let mask_type_result = mask_type_response.result().cloned().expect("mask type result");
    let mask_type_definition = serde_json::from_value::<Option<GotoDefinitionResponse>>(mask_type_result)
        .expect("deserialize mask type goto-definition payload")
        .expect("AttnMaskParams definition should be available");
    let mask_type_target = first_location(mask_type_definition);
    assert!(
        mask_type_target.uri.path().ends_with("generated/attention.h"),
        "AttnMaskParams should resolve to generated attention header, got {}",
        mask_type_target.uri.path()
    );

    // 3) `v_strides` field access should resolve to field declaration in header.
    let field_params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.clone(),
            },
            position: {
                let mut pos = position_for("params->v_strides[1]");
                pos.character += "params->".chars().count() as u32;
                pos
            },
        },
        work_done_progress_params: WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: PartialResultParams {
            partial_result_token: None,
        },
    };

    let (field_response, _) = send_request_with_client_pump(
        &mut service,
        &mut socket,
        &mut pending_notifications,
        "textDocument/definition",
        field_params,
        4,
    )
    .await;
    assert!(
        field_response.is_ok(),
        "v_strides goto-definition should succeed, got error: {:?}",
        field_response.error()
    );
    let field_result = field_response.result().cloned().expect("field result");
    let field_definition = serde_json::from_value::<Option<GotoDefinitionResponse>>(field_result)
        .expect("deserialize field goto-definition payload")
        .expect("v_strides definition should be available");
    let field_target = first_location(field_definition);
    assert!(
        field_target.uri.path().ends_with("generated/attention.h"),
        "v_strides should resolve to generated attention header, got {}",
        field_target.uri.path()
    );

    // 4) `o_strides` field access should resolve to field declaration in header.
    let out_field_params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.clone(),
            },
            position: {
                let mut pos = position_for("params->o_strides[2]");
                pos.character += "params->".chars().count() as u32;
                pos
            },
        },
        work_done_progress_params: WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: PartialResultParams {
            partial_result_token: None,
        },
    };

    let (out_field_response, _) = send_request_with_client_pump(
        &mut service,
        &mut socket,
        &mut pending_notifications,
        "textDocument/definition",
        out_field_params,
        5,
    )
    .await;
    assert!(
        out_field_response.is_ok(),
        "o_strides goto-definition should succeed, got error: {:?}",
        out_field_response.error()
    );
    let out_field_result = out_field_response.result().cloned().expect("o_strides result");
    let out_field_definition = serde_json::from_value::<Option<GotoDefinitionResponse>>(out_field_result)
        .expect("deserialize o_strides goto-definition payload")
        .expect("o_strides definition should be available");
    let out_field_target = first_location(out_field_definition);
    assert!(
        out_field_target.uri.path().ends_with("generated/attention.h"),
        "o_strides should resolve to generated attention header, got {}",
        out_field_target.uri.path()
    );
}
