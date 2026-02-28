use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{Arc, atomic::AtomicU64},
};

use dashmap::DashMap;
use tokio::sync::RwLock;
use tower_lsp::{
    Client,
    lsp_types::{Diagnostic, Url, WorkspaceFolder},
};

use crate::{
    completion::CompletionProvider, definition::DefinitionProvider, document::DocumentStore, hover::HoverProvider,
    metal::compiler::MetalCompiler, semantic_tokens::SemanticTokenProvider, server::settings::ServerSettings,
    symbols::SymbolProvider, syntax::DocumentTrees,
};

/// The metal-analyzer backend that implements the Language Server Protocol.
pub struct MetalLanguageServer {
    /// The LSP client handle, used to send notifications (e.g. diagnostics) back.
    pub(crate) client: Client,

    /// Thread-safe store of all open documents.
    pub(crate) document_store: Arc<DocumentStore>,

    /// Invokes `xcrun metal` and parses compiler diagnostics.
    pub(crate) compiler: Arc<MetalCompiler>,

    /// Provides auto-completion items.
    pub(crate) completion_provider: Arc<CompletionProvider>,

    /// Provides hover information for Metal symbols.
    pub(crate) hover_provider: Arc<HoverProvider>,

    /// Provides go-to-definition via the Metal compiler's AST dump.
    pub(crate) definition_provider: Arc<DefinitionProvider>,

    /// Provides semantic tokens for syntax highlighting.
    pub(crate) semantic_token_provider: Arc<SemanticTokenProvider>,

    /// Provides fast regex-based symbol extraction for document symbols.
    pub(crate) symbol_provider: Arc<SymbolProvider>,

    /// Rowan-parsed syntax trees for all open documents.
    pub(crate) document_trees: Arc<DocumentTrees>,

    /// Workspace root folders, populated during `initialize`.
    pub(crate) workspace_roots: RwLock<Vec<WorkspaceFolder>>,

    /// Per-document diagnostics cache so we can clear them on close.
    pub(crate) diagnostics_cache: DashMap<Url, Vec<Diagnostic>>,

    /// Monotonic per-document generation for diagnostics runs.
    ///
    /// Incremented on every diagnostics request so stale async compiler results
    /// can be dropped instead of overwriting newer editor state.
    pub(crate) diagnostics_generation: Arc<DashMap<Url, u64>>,

    /// Reverse include graph: header file -> owner `.metal` files that include it.
    pub(crate) header_owners: Arc<DashMap<PathBuf, BTreeSet<PathBuf>>>,

    /// Forward include graph: owner `.metal` file -> included header files.
    pub(crate) owner_headers: Arc<DashMap<PathBuf, BTreeSet<PathBuf>>>,

    /// Monotonic generation for goto-definition requests.
    ///
    /// Bumped on every new request so in-flight AST dumps for stale requests
    /// can be abandoned early instead of blocking newer jumps.
    pub(crate) goto_def_generation: Arc<AtomicU64>,

    /// Debounce generation counter for background AST indexing on edits.
    ///
    /// We bump the counter on every `did_change` and only run the expensive
    /// AST dump for the latest generation after a short idle delay.
    pub(crate) ast_index_generation: Arc<DashMap<Url, u64>>,

    /// Memoized include-path lists per source file path.
    ///
    /// Value format: `(workspace_generation, include_paths)`.
    pub(crate) include_paths_cache: Arc<DashMap<PathBuf, (u64, Vec<String>)>>,

    /// Monotonic generation for workspace-root changes.
    ///
    /// Bumping this invalidates stale include-path cache entries.
    pub(crate) workspace_generation: Arc<AtomicU64>,

    /// Runtime server settings updated from LSP configuration.
    pub(crate) settings: Arc<RwLock<ServerSettings>>,
}

impl MetalLanguageServer {
    /// Create a new `MetalLanguageServer` wired to the given LSP client.
    ///
    /// `_log_messages` is accepted for CLI compatibility but message-level
    /// logging is controlled entirely through the `tracing` subscriber.
    pub fn new(
        client: Client,
        _log_messages: bool,
    ) -> Self {
        let document_store = Arc::new(DocumentStore::new());
        let compiler = Arc::new(MetalCompiler::new());
        let completion_provider = Arc::new(CompletionProvider::new());
        let symbol_provider = Arc::new(SymbolProvider::new());
        let definition_provider = Arc::new(DefinitionProvider::new());
        let hover_provider =
            Arc::new(HoverProvider::new(Arc::clone(&symbol_provider), Arc::clone(&definition_provider)));
        let semantic_token_provider = Arc::new(SemanticTokenProvider::new(Arc::clone(&definition_provider)));
        let document_trees = Arc::new(DocumentTrees::new());
        let diagnostics_generation = Arc::new(DashMap::new());
        let header_owners = Arc::new(DashMap::new());
        let owner_headers = Arc::new(DashMap::new());
        let ast_index_generation = Arc::new(DashMap::new());
        let include_paths_cache = Arc::new(DashMap::new());
        let goto_def_generation = Arc::new(AtomicU64::new(0));
        let workspace_generation = Arc::new(AtomicU64::new(0));
        let settings = Arc::new(RwLock::new(ServerSettings::default()));

        Self {
            client,
            document_store,
            compiler,
            completion_provider,
            hover_provider,
            definition_provider,
            semantic_token_provider,
            symbol_provider,
            document_trees,
            workspace_roots: RwLock::new(Vec::new()),
            diagnostics_cache: DashMap::new(),
            diagnostics_generation,
            header_owners,
            owner_headers,
            goto_def_generation,
            ast_index_generation,
            include_paths_cache,
            workspace_generation,
            settings,
        }
    }

    pub(crate) async fn settings_snapshot(&self) -> ServerSettings {
        self.settings.read().await.clone()
    }

    pub(crate) async fn apply_settings(
        &self,
        settings: ServerSettings,
    ) {
        let include_paths = settings.compiler.include_paths.iter().map(PathBuf::from).collect::<Vec<_>>();
        self.compiler.set_include_paths(include_paths);
        self.compiler.set_flags(settings.compiler.extra_flags.clone());
        self.compiler.set_platform(settings.compiler.platform);

        *self.settings.write().await = settings;
    }
}
