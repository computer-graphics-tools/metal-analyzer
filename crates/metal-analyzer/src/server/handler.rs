use std::{panic::AssertUnwindSafe, sync::atomic::Ordering, time::Duration};

use futures::FutureExt;
use tower_lsp::{LanguageServer, jsonrpc::Result, lsp_types::*};
use tracing::{debug, info, warn};

use crate::{
    ide::lsp::{ide_location_to_lsp, ide_range_to_lsp, navigation_target_to_lsp},
    progress::ProgressToken,
    semantic_tokens::get_legend,
    server::{
        diagnostics::{compile_filtered_diagnostics_for_document, compute_include_paths_for_uri_cached},
        formatting::{FormattingError, format_document},
        header_owners::{collect_included_headers, update_owner_links},
        settings::ServerSettings,
        state::MetalLanguageServer,
    },
    syntax::SyntaxTree,
};

const CLIENT_NOTIFICATION_PREFIX: &str = "metal-analyzer:";

#[tower_lsp::async_trait]
impl LanguageServer for MetalLanguageServer {
    async fn initialize(
        &self,
        params: InitializeParams,
    ) -> Result<InitializeResult> {
        info!("Initializing metal-analyzer...");

        let initial_settings = ServerSettings::from_lsp_payload(params.initialization_options.as_ref());
        self.apply_settings(initial_settings).await;

        if let Some(folders) = params.workspace_folders {
            *self.workspace_roots.write().await = folders;
        } else if let Some(root) = params.root_uri {
            *self.workspace_roots.write().await = vec![WorkspaceFolder {
                uri: root,
                name: "root".to_string(),
            }];
        }
        self.workspace_generation.fetch_add(1, Ordering::Relaxed);
        self.include_paths_cache.clear();

        // Kick off system include discovery in background
        let compiler = self.compiler.clone();
        tokio::spawn(async move {
            compiler.discover_system_includes().await;
        });

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::INCREMENTAL)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_string(), ":".to_string(), "#".to_string()]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                declaration_provider: Some(DeclarationCapability::Simple(true)),
                type_definition_provider: Some(TypeDefinitionProviderCapability::Simple(true)),
                implementation_provider: Some(ImplementationProviderCapability::Simple(true)),
                references_provider: Some(OneOf::Left(true)),
                document_highlight_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: Default::default(),
                })),
                semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
                    SemanticTokensOptions {
                        legend: get_legend(),
                        full: Some(SemanticTokensFullOptions::Bool(true)),
                        range: Some(false),
                        work_done_progress_options: Default::default(),
                    },
                )),
                document_formatting_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "metal-analyzer".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(
        &self,
        _: InitializedParams,
    ) {
        info!("metal-analyzer initialized");

        let settings = self.settings_snapshot().await;
        let should_scan_workspace = settings.indexing.enable || settings.diagnostics.scope.is_workspace();
        if !should_scan_workspace {
            info!(
                "Skipping workspace scan because metal-analyzer.indexing.enabled=false and \
                 metal-analyzer.diagnostics.scope=openFiles"
            );
            return;
        }

        // Kick off project-wide indexing / diagnostics in background.
        let handle = self.clone_for_background().await;
        tokio::spawn(async move {
            handle.index_workspace().await;
        });
    }

    async fn did_change_configuration(
        &self,
        params: DidChangeConfigurationParams,
    ) {
        let current = self.settings_snapshot().await;
        let merged = current.merged_with_payload(&params.settings);
        if merged == current {
            return;
        }

        let workspace_scan_enabled_after_change = merged.indexing.enable || merged.diagnostics.scope.is_workspace();
        let scope_became_workspace =
            !current.diagnostics.scope.is_workspace() && merged.diagnostics.scope.is_workspace();
        let indexing_inputs_changed = merged.indexing != current.indexing && merged.indexing.enable;
        let compiler_inputs_changed = merged.compiler != current.compiler;
        let should_start_workspace_scan = workspace_scan_enabled_after_change
            && (scope_became_workspace || indexing_inputs_changed || compiler_inputs_changed);
        self.apply_settings(merged).await;
        self.workspace_generation.fetch_add(1, Ordering::Relaxed);
        self.include_paths_cache.clear();
        info!("Applied updated metal-analyzer settings");

        if should_start_workspace_scan {
            let handle = self.clone_for_background().await;
            tokio::spawn(async move {
                handle.index_workspace().await;
            });
        }
    }

    async fn shutdown(&self) -> Result<()> {
        info!("Shutting down metal-analyzer");
        self.client
            .show_message(
                MessageType::WARNING,
                prefixed_client_message(
                    "Shutting down. Language features will be unavailable until the server restarts.",
                ),
            )
            .await;
        Ok(())
    }

    async fn did_open(
        &self,
        params: DidOpenTextDocumentParams,
    ) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        let version = params.text_document.version;
        let filename = short_name(&uri);
        let settings = self.settings_snapshot().await;
        let diagnostics_on_type = settings.diagnostics.on_type;
        let indexing_enabled = settings.indexing.enable;
        let allow_client_info_logs = settings.logging.level.allows_info();

        info!("Opened {filename} (v{version}, {} bytes)", text.len());
        if allow_client_info_logs {
            let _ = AssertUnwindSafe(
                self.client.log_message(MessageType::INFO, prefixed_client_message(format!("Opened {filename}"))),
            )
            .catch_unwind()
            .await;
        }

        // Lightweight synchronous work only.
        self.document_store.open(uri.clone(), text.clone(), version);
        let tree = SyntaxTree::parse(&text);
        self.document_trees.insert(uri.clone(), tree.clone());
        self.symbol_provider.scan_file(&uri, &text);

        // Heavy work (include paths, diagnostics, AST indexing) in background
        // so the editor gets a response immediately.
        let provider = self.definition_provider.clone();
        let compiler = self.compiler.clone();
        let client = self.client.clone();
        let workspace_roots =
            self.workspace_roots.read().await.iter().filter_map(|f| f.uri.to_file_path().ok()).collect::<Vec<_>>();
        let header_owners = self.header_owners.clone();
        let owner_headers = self.owner_headers.clone();
        let document_store = self.document_store.clone();
        let diagnostics_generation = self.diagnostics_generation.clone();
        let include_paths_cache = self.include_paths_cache.clone();
        let workspace_generation = self.workspace_generation.load(Ordering::Relaxed);
        let fname = filename.clone();

        let diagnostics_generation_value = if diagnostics_on_type {
            let mut g = diagnostics_generation.entry(uri.clone()).or_insert(0);
            *g += 1;
            Some(*g)
        } else {
            None
        };

        tokio::spawn(async move {
            // Compute include paths once.
            compiler.ensure_system_includes_ready().await;
            let includes = compute_include_paths_for_uri_cached(
                &compiler,
                &uri,
                &workspace_roots,
                &include_paths_cache,
                workspace_generation,
            )
            .await;

            // Header ownership.
            if let Ok(path) = uri.to_file_path()
                && path.extension().is_some_and(|ext| ext == "metal")
            {
                let headers = collect_included_headers(&path, &text, &includes);
                update_owner_links(&header_owners, &owner_headers, &path, headers);
            }

            // Diagnostics.
            if let Some(generation) = diagnostics_generation_value {
                if let Some(doc) = document_store.get(&uri) {
                    let diagnostics = compile_filtered_diagnostics_for_document(
                        &compiler,
                        &workspace_roots,
                        &header_owners,
                        &owner_headers,
                        &include_paths_cache,
                        workspace_generation,
                        &uri,
                        &doc.text,
                    )
                    .await;

                    let still_latest = diagnostics_generation.get(&uri).is_some_and(|current| *current == generation);
                    if still_latest {
                        let result =
                            AssertUnwindSafe(client.publish_diagnostics(uri.clone(), diagnostics, Some(version)))
                                .catch_unwind()
                                .await;
                        if result.is_err() {
                            warn!("publish_diagnostics panicked (client may have disconnected)");
                        }
                    }
                }
            }

            // AST indexing.
            if indexing_enabled {
                provider.index_document(&uri, &text, &includes);
                if allow_client_info_logs {
                    let _ =
                        AssertUnwindSafe(client.log_message(
                            MessageType::INFO,
                            prefixed_client_message(format!("Indexed AST for {fname}")),
                        ))
                        .catch_unwind()
                        .await;
                }
            }
        });
    }

    async fn did_change(
        &self,
        params: DidChangeTextDocumentParams,
    ) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;

        self.document_store.apply_changes(&uri, params.content_changes, version);

        let Some(text) = self.document_store.get_content(&uri) else {
            return;
        };
        let settings = self.settings_snapshot().await;
        let diagnostics_on_type = settings.diagnostics.on_type;
        let diagnostics_debounce_ms = settings.diagnostics.debounce_ms;
        let indexing_enabled = settings.indexing.enable;

        // Lightweight synchronous work only: parse tree + symbol scan.
        let tree = SyntaxTree::parse(&text);
        self.document_trees.insert(uri.clone(), tree.clone());
        self.symbol_provider.scan_file(&uri, &text);

        // Bump debounce generation for both AST indexing and diagnostics.
        let ast_generation = if indexing_enabled {
            let mut g = self.ast_index_generation.entry(uri.clone()).or_insert(0);
            *g += 1;
            Some(*g)
        } else {
            None
        };
        let diag_generation = if diagnostics_on_type {
            let mut g = self.diagnostics_generation.entry(uri.clone()).or_insert(0);
            *g += 1;
            Some(*g)
        } else {
            None
        };

        // Clone shared state for the single debounced background task.
        let ast_gen_map = self.ast_index_generation.clone();
        let diag_gen_map = self.diagnostics_generation.clone();
        let provider = self.definition_provider.clone();
        let compiler = self.compiler.clone();
        let client = self.client.clone();
        let workspace_roots =
            self.workspace_roots.read().await.iter().filter_map(|f| f.uri.to_file_path().ok()).collect::<Vec<_>>();
        let header_owners = self.header_owners.clone();
        let owner_headers = self.owner_headers.clone();
        let include_paths_cache = self.include_paths_cache.clone();
        let workspace_generation = self.workspace_generation.load(Ordering::Relaxed);

        // Single debounced task: include paths, header ownership, AST index,
        // and diagnostics all run together after the idle delay so we avoid
        // computing include paths on every keystroke.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(diagnostics_debounce_ms)).await;

            // Check both generations — bail out if a newer change arrived.
            let ast_current =
                ast_generation.is_some_and(|generation| ast_gen_map.get(&uri).is_some_and(|g| *g == generation));
            let diag_current =
                diag_generation.is_some_and(|generation| diag_gen_map.get(&uri).is_some_and(|g| *g == generation));
            if !ast_current && !diag_current {
                return;
            }

            // Compute include paths once (the expensive part).
            compiler.ensure_system_includes_ready().await;
            let includes = compute_include_paths_for_uri_cached(
                &compiler,
                &uri,
                &workspace_roots,
                &include_paths_cache,
                workspace_generation,
            )
            .await;

            // Update header ownership links (for .metal files).
            if let Ok(path) = uri.to_file_path()
                && path.extension().is_some_and(|ext| ext == "metal")
            {
                let headers = collect_included_headers(&path, &text, &includes);
                update_owner_links(&header_owners, &owner_headers, &path, headers);
            }

            // AST indexing (if still current).
            if ast_current {
                provider.index_document(&uri, &text, &includes);
            }

            // Diagnostics (if still current).
            if let Some(diag_generation) = diag_generation.filter(|_| diag_current) {
                let diagnostics = compile_filtered_diagnostics_for_document(
                    &compiler,
                    &workspace_roots,
                    &header_owners,
                    &owner_headers,
                    &include_paths_cache,
                    workspace_generation,
                    &uri,
                    &text,
                )
                .await;

                // Re-check staleness after compilation.
                let still_latest = diag_gen_map.get(&uri).is_some_and(|current| *current == diag_generation);
                if still_latest {
                    let result = AssertUnwindSafe(client.publish_diagnostics(uri, diagnostics, Some(version)))
                        .catch_unwind()
                        .await;
                    if result.is_err() {
                        warn!("publish_diagnostics panicked (client may have disconnected)");
                    }
                }
            }
        });
    }

    async fn did_save(
        &self,
        params: DidSaveTextDocumentParams,
    ) {
        let uri = params.text_document.uri;
        let filename = short_name(&uri);
        let settings = self.settings_snapshot().await;
        debug!("Saved {filename}");

        if settings.diagnostics.on_save {
            self.run_diagnostics(&uri).await;
        }

        if let Some(text) = self.document_store.get_content(&uri) {
            let provider = self.definition_provider.clone();
            let includes = self.include_paths(&uri).await;
            if let Ok(path) = uri.to_file_path()
                && path.extension().is_some_and(|ext| ext == "metal")
            {
                let headers = collect_included_headers(&path, &text, &includes);
                update_owner_links(&self.header_owners, &self.owner_headers, &path, headers);
            }
            let file_path = uri.to_file_path().ok();
            let client = self.client.clone();
            let fname = filename.clone();
            let allow_client_info_logs = settings.logging.level.allows_info();
            let indexing_enabled = settings.indexing.enable;
            tokio::spawn(async move {
                if indexing_enabled {
                    provider.index_document(&uri, &text, &includes);
                    if let Some(path) = file_path {
                        provider.index_workspace_file(&path, &includes);
                    }
                }
                if allow_client_info_logs {
                    let _ = AssertUnwindSafe(
                        client.log_message(MessageType::INFO, prefixed_client_message(format!("Re-indexed {fname}"))),
                    )
                    .catch_unwind()
                    .await;
                }
            });
        }
    }

    async fn did_close(
        &self,
        params: DidCloseTextDocumentParams,
    ) {
        let uri = params.text_document.uri;
        let keep_workspace_diagnostics = self.settings_snapshot().await.diagnostics.scope.is_workspace();
        if let Ok(path) = uri.to_file_path() {
            let cache_key = path.canonicalize().unwrap_or(path);
            self.include_paths_cache.remove(&cache_key);
        }
        self.document_store.close(&uri);
        self.document_trees.remove(&uri);
        self.symbol_provider.remove_file(&uri);
        if keep_workspace_diagnostics {
            self.diagnostics_cache.remove(&uri);
            self.diagnostics_generation.remove(&uri);
        } else {
            self.clear_diagnostics(&uri).await;
        }
        self.definition_provider.evict(&uri);
        self.ast_index_generation.remove(&uri);
    }

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let text = self.document_store.get_content(&uri);
        let tree = self.document_trees.get(&uri);

        let items = self.completion_provider.provide(text.as_deref(), position, tree.as_ref());
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let Some(document) = self.document_store.get(&uri) else {
            return Ok(None);
        };
        let settings = self.settings_snapshot().await;
        if !settings.formatting.enable {
            return Ok(Some(Vec::new()));
        }

        match format_document(&document, &params.options, &settings.formatting).await {
            Ok(Some(edit)) => Ok(Some(vec![edit])),
            Ok(None) => Ok(Some(Vec::new())),
            Err(error) => {
                warn!("Formatting failed for {uri}: {error}");
                match error {
                    FormattingError::CommandNotFound(command) => {
                        self.client
                            .show_message(
                                MessageType::WARNING,
                                prefixed_client_message(format!(
                                    "Formatting command '{command}' is not available. Install it or update metal-analyzer.formatting.command."
                                )),
                            )
                            .await;
                    },
                    _ => {
                        self.client
                            .show_message(
                                MessageType::WARNING,
                                prefixed_client_message(format!("Formatting failed: {error}")),
                            )
                            .await;
                    },
                }
                Ok(None)
            },
        }
    }

    async fn hover(
        &self,
        params: HoverParams,
    ) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let text = match self.document_store.get_content(&uri) {
            Some(t) => t,
            None => return Ok(None),
        };
        let tree = self.document_trees.get(&uri);

        Ok(self.hover_provider.provide(&uri, &text, position, tree.as_ref()).await)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let text = match self.document_store.get_content(&uri) {
            Some(t) => t,
            None => return Ok(None),
        };
        let tree = self.document_trees.get(&uri).unwrap_or_else(|| SyntaxTree::parse(&text));

        let generation = self.goto_def_generation.fetch_add(1, Ordering::Relaxed) + 1;
        let gen_ref = self.goto_def_generation.clone();
        let is_cancelled = move || gen_ref.load(Ordering::Relaxed) != generation;

        let progress = ProgressToken::begin(&self.client, "Definition", Some("Finding definition…".to_string())).await;
        let include_start = std::time::Instant::now();
        let includes = self.include_paths(&uri).await;
        let include_elapsed = include_start.elapsed();

        if is_cancelled() {
            debug!("goto-def cancelled before provide");
            progress.end(Some("Cancelled".to_string())).await;
            return Ok(None);
        }

        let start = std::time::Instant::now();
        let nav_result = self.definition_provider.provide(&uri, position, &text, &includes, &tree, &is_cancelled);
        let elapsed = start.elapsed();

        let filename = short_name(&uri);
        debug!(
            "goto-def include-paths {filename}:{}:{} prepared in {:?}",
            position.line + 1,
            position.character + 1,
            include_elapsed,
        );

        let lsp_result = nav_result.and_then(navigation_target_to_lsp);

        match &lsp_result {
            Some(resp) => {
                let target = match resp {
                    GotoDefinitionResponse::Scalar(loc) => {
                        format!("{}:{}", short_path(loc.uri.path()), loc.range.start.line + 1)
                    },
                    GotoDefinitionResponse::Array(locs) => format!("{} locations", locs.len()),
                    GotoDefinitionResponse::Link(links) => format!("{} links", links.len()),
                };
                debug!("goto-def {filename}:{}:{} → {target} ({elapsed:?})", position.line + 1, position.character + 1,);
                progress.end(Some(format!("Resolved definition: {target}"))).await;
            },
            None => {
                debug!("goto-def {filename}:{}:{} → none ({elapsed:?})", position.line + 1, position.character + 1,);
                progress.end(Some("No definition found".to_string())).await;
            },
        }
        Ok(lsp_result)
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let tree = self.document_trees.get(&uri);

        let tokens = self.semantic_token_provider.provide(&uri, tree.as_ref());
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        })))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let symbols = self.symbol_provider.document_symbols(&uri);
        Ok(Some(DocumentSymbolResponse::Flat(symbols)))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        Ok(Some(self.symbol_provider.workspace_symbols(&params.query)))
    }

    async fn goto_declaration(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let text = match self.document_store.get_content(&uri) {
            Some(t) => t,
            None => return Ok(None),
        };
        let tree = self.document_trees.get(&uri).unwrap_or_else(|| SyntaxTree::parse(&text));
        let includes = self.include_paths(&uri).await;

        let result = self.definition_provider.provide_declaration(&uri, position, &text, &includes, &tree);
        Ok(result.and_then(navigation_target_to_lsp))
    }

    async fn goto_type_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let text = match self.document_store.get_content(&uri) {
            Some(t) => t,
            None => return Ok(None),
        };
        let tree = self.document_trees.get(&uri).unwrap_or_else(|| SyntaxTree::parse(&text));
        let includes = self.include_paths(&uri).await;

        let result = self.definition_provider.provide_type_definition(&uri, position, &text, &includes, &tree);
        Ok(result.and_then(navigation_target_to_lsp))
    }

    async fn goto_implementation(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let text = match self.document_store.get_content(&uri) {
            Some(t) => t,
            None => return Ok(None),
        };
        let tree = self.document_trees.get(&uri).unwrap_or_else(|| SyntaxTree::parse(&text));
        let includes = self.include_paths(&uri).await;

        let result = self.definition_provider.provide_implementation(&uri, position, &text, &includes, &tree);
        Ok(result.and_then(navigation_target_to_lsp))
    }

    async fn references(
        &self,
        params: ReferenceParams,
    ) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let text = match self.document_store.get_content(&uri) {
            Some(t) => t,
            None => return Ok(None),
        };
        let tree = self.document_trees.get(&uri).unwrap_or_else(|| SyntaxTree::parse(&text));
        let includes = self.include_paths(&uri).await;

        let result = self.definition_provider.provide_references(
            &uri,
            position,
            &text,
            &includes,
            &tree,
            params.context.include_declaration,
        );
        Ok(result.map(|locs| locs.into_iter().filter_map(ide_location_to_lsp).collect()))
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let text = match self.document_store.get_content(&uri) {
            Some(t) => t,
            None => return Ok(None),
        };
        let tree = self.document_trees.get(&uri).unwrap_or_else(|| SyntaxTree::parse(&text));
        let includes = self.include_paths(&uri).await;

        let refs = self.definition_provider.provide_references(&uri, position, &text, &includes, &tree, true);

        Ok(refs.map(|locs| {
            locs.into_iter()
                .filter_map(ide_location_to_lsp)
                .filter(|l| l.uri == uri)
                .map(|l| DocumentHighlight {
                    range: l.range,
                    kind: None,
                })
                .collect()
        }))
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let position = params.position;
        let text = match self.document_store.get_content(&uri) {
            Some(t) => t,
            None => return Ok(None),
        };
        let tree = self.document_trees.get(&uri).unwrap_or_else(|| SyntaxTree::parse(&text));
        let includes = self.include_paths(&uri).await;

        let range = self.definition_provider.prepare_rename(&uri, position, &text, &includes, &tree);
        Ok(range.map(|r| PrepareRenameResponse::Range(ide_range_to_lsp(r))))
    }

    async fn rename(
        &self,
        params: RenameParams,
    ) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let new_name = params.new_name;

        let text = match self.document_store.get_content(&uri) {
            Some(t) => t,
            None => return Ok(None),
        };
        let tree = self.document_trees.get(&uri).unwrap_or_else(|| SyntaxTree::parse(&text));
        let includes = self.include_paths(&uri).await;

        let refs = self.definition_provider.provide_references(&uri, position, &text, &includes, &tree, true);

        if let Some(ide_locations) = refs {
            let mut changes = std::collections::HashMap::new();
            for ide_loc in ide_locations {
                if let Some(lsp_loc) = ide_location_to_lsp(ide_loc) {
                    changes.entry(lsp_loc.uri).or_insert_with(Vec::new).push(TextEdit {
                        range: lsp_loc.range,
                        new_text: new_name.clone(),
                    });
                }
            }
            let edit = WorkspaceEdit {
                changes: Some(changes),
                document_changes: None,
                change_annotations: None,
            };
            Ok(Some(edit))
        } else {
            Ok(None)
        }
    }
}

fn short_name(uri: &Url) -> String {
    uri.path().rsplit('/').next().unwrap_or(uri.path()).to_owned()
}

fn short_path(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn prefixed_client_message(message: impl AsRef<str>) -> String {
    format!("{CLIENT_NOTIFICATION_PREFIX} {}", message.as_ref())
}
