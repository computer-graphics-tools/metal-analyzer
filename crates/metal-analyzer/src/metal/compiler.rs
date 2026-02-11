use regex::Regex;
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use tokio::process::Command;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use tracing::{debug, error, warn};

static NEXT_COMPILATION_ID: AtomicU64 = AtomicU64::new(1);
const METAL_MACOS_DEFINE: &str = "-D__METAL_MACOS__";
const METAL_IOS_DEFINE: &str = "-D__METAL_IOS__";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CompilerPlatform {
    #[default]
    Auto,
    Macos,
    Ios,
    None,
}

impl CompilerPlatform {
    pub(crate) fn from_setting_value(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Self::Auto,
            "macos" => Self::Macos,
            "ios" => Self::Ios,
            "none" => Self::None,
            _ => Self::Auto,
        }
    }

    pub(crate) fn as_setting_value(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Macos => "macos",
            Self::Ios => "ios",
            Self::None => "none",
        }
    }

    fn default_injected_define(self) -> Option<&'static str> {
        match self {
            Self::Auto | Self::Macos => Some(METAL_MACOS_DEFINE),
            Self::Ios => Some(METAL_IOS_DEFINE),
            Self::None => None,
        }
    }
}

fn xcrun_command() -> Command {
    let mut command = Command::new("xcrun");
    command.kill_on_drop(true);
    command
}

// ---------------------------------------------------------------------------
// Include path discovery
// ---------------------------------------------------------------------------

/// Compute include search paths for a Metal source file.
///
/// Returns a list of directories that should be passed as `-I` flags to the
/// Metal compiler. The paths include:
/// - All ancestor directories of the file (up to workspace roots)
/// - Immediate child directories of those ancestors (e.g., `generated/`, `common/`)
///
/// Paths are filtered to exclude hidden directories, build artifacts, and
/// common non-source directories. The result is ordered with the most specific
/// (deepest) paths first.
///
/// # Arguments
/// * `file_path` - The path to the Metal source file
/// * `workspace_roots` - Optional list of workspace root paths. If provided,
///   ancestor walking stops at the nearest workspace root. If `None`, walks
///   all the way to the filesystem root.
pub fn compute_include_paths(file_path: &Path, workspace_roots: Option<&[PathBuf]>) -> Vec<String> {
    let mut unique = BTreeSet::new();

    // Find the workspace root that contains this file (if any)
    let workspace_root = workspace_roots.and_then(|roots| {
        roots
            .iter()
            .find(|root| file_path.starts_with(root))
            .cloned()
    });

    // Helper to add a directory and its immediate children
    let add_dir_and_children = |dir: &Path, unique: &mut BTreeSet<String>| {
        // Add this directory
        unique.insert(dir.to_string_lossy().into_owned());

        // Add immediate child directories
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let child_path = entry.path();
                if child_path.is_dir() {
                    let name = child_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");
                    // Filter out unwanted directories
                    if !should_exclude_dir(name) {
                        unique.insert(child_path.to_string_lossy().into_owned());
                    }
                }
            }
        }
    };

    // Walk up ancestors, stopping at workspace root or filesystem root
    if let Some(mut current) = file_path.parent() {
        loop {
            add_dir_and_children(current, &mut unique);

            // Check if we've reached the workspace root
            if let Some(ref ws_root) = workspace_root
                && current == ws_root.as_path()
            {
                // Include the workspace root and its children, then stop
                break;
            }

            // Move to parent
            match current.parent() {
                Some(parent) if parent != current => {
                    current = parent;
                }
                _ => break,
            }
        }
    }

    // Convert to Vec, ordering by depth (deepest first)
    // We collect paths with their depth, sort by depth descending, then extract paths
    let mut paths_with_depth: Vec<(usize, String)> = unique
        .into_iter()
        .map(|path| {
            let depth = path.matches(std::path::MAIN_SEPARATOR).count();
            (depth, path)
        })
        .collect();

    // Sort by depth descending (deepest first), then lexicographically for same depth
    paths_with_depth.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

    paths_with_depth.into_iter().map(|(_, path)| path).collect()
}

/// Check if a directory name should be excluded from include path discovery.
fn should_exclude_dir(name: &str) -> bool {
    // Hidden directories
    if name.starts_with('.') {
        return true;
    }

    // Common build/artifact directories
    matches!(
        name,
        "target" | "build" | "node_modules" | ".git" | ".cargo" | "out" | "bin" | "obj"
    )
}

fn parse_include_search_paths(raw_output: &str) -> Vec<PathBuf> {
    let mut parsing_includes = false;
    let mut discovered_paths = Vec::new();

    for line in raw_output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("#include <...> search starts here:") {
            parsing_includes = true;
            continue;
        }
        if !parsing_includes {
            continue;
        }
        if trimmed.starts_with("End of search list.") {
            break;
        }
        if trimmed.is_empty() {
            continue;
        }

        // Clang may annotate framework roots as " (framework directory)".
        let path_text = trimmed
            .trim_end_matches(" (framework directory)")
            .trim_matches('"');
        if let Some(path) = normalize_existing_path(PathBuf::from(path_text)) {
            discovered_paths.push(path);
        }
    }

    dedupe_existing_paths(discovered_paths)
}

fn fallback_include_paths_from_toolchain_signature(toolchain_signature: Option<&str>) -> Vec<PathBuf> {
    let Some(signature) = toolchain_signature else {
        return Vec::new();
    };

    let metal_binary = PathBuf::from(signature);
    let mut discovered_paths = Vec::new();

    if let Some(bin_dir) = metal_binary.parent()
        && let Some(metal_version_dir) = bin_dir.parent()
    {
        // Typical layout:
        //   .../usr/metal/<version>/bin/metal
        //   .../usr/metal/<version>/lib/clang/<clang-version>/include
        discovered_paths.extend(clang_include_dirs_under(&metal_version_dir.join("lib/clang")));

        // Some distributions may expose include directly under lib/clang/include.
        if let Some(path) = normalize_existing_path(metal_version_dir.join("lib/clang/include")) {
            discovered_paths.push(path);
        }
    }

    dedupe_existing_paths(discovered_paths)
}

fn clang_include_dirs_under(clang_root: &Path) -> Vec<PathBuf> {
    let mut include_dirs = Vec::new();
    let Ok(entries) = std::fs::read_dir(clang_root) else {
        return include_dirs;
    };

    for entry in entries.flatten() {
        let candidate = entry.path().join("include");
        if let Some(path) = normalize_existing_path(candidate) {
            include_dirs.push(path);
        }
    }

    include_dirs
}

fn normalize_existing_path(path: PathBuf) -> Option<PathBuf> {
    if !path.exists() {
        return None;
    }
    Some(path.canonicalize().unwrap_or(path))
}

fn dedupe_existing_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for path in paths {
        if seen.insert(path.clone()) {
            deduped.push(path);
        }
    }
    deduped
}

/// Represents a parsed diagnostic from the Metal compiler output.
#[derive(Debug, Clone)]
pub struct MetalDiagnostic {
    pub file: Option<String>,
    pub line: u32,
    pub column: u32,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

impl MetalDiagnostic {
    /// Convert into an LSP `Diagnostic`.
    pub fn into_lsp_diagnostic(self) -> Diagnostic {
        let pos = Position::new(self.line, self.column);
        Diagnostic {
            range: Range::new(pos, pos),
            severity: Some(self.severity),
            code: None,
            code_description: None,
            source: Some("metal-compiler".to_string()),
            message: self.message,
            related_information: None,
            tags: None,
            data: None,
        }
    }
}

/// Manages invocation of the Metal shader compiler (`xcrun metal`)
/// and parsing of its diagnostic output.
///
/// Supports configurable include paths and extra compiler flags so that
/// projects with deep include hierarchies (for example using paths such
/// as `#include "../../../common/utils.h"`) work correctly.
pub struct MetalCompiler {
    /// Temporary directory for compilation artifacts.
    temp_dir: PathBuf,
    /// Compiled regex for parsing diagnostic lines.
    diagnostic_re: Regex,
    /// System include paths discovered from the toolchain.
    system_include_paths: RwLock<Vec<PathBuf>>,
    /// Extra include search paths registered via configuration or workspace roots.
    extra_include_paths: RwLock<Vec<PathBuf>>,
    /// Extra compiler flags forwarded verbatim (e.g. `-std=metal3.1`, `-DFOO`).
    extra_flags: RwLock<Vec<String>>,
    /// Platform context used to resolve implicit compiler defines.
    platform: RwLock<CompilerPlatform>,
    /// Serializes first-time include discovery so startup races don't trigger
    /// duplicate discovery calls.
    include_discovery_lock: tokio::sync::Mutex<()>,
    /// Canonical path to the active `xcrun --find metal` binary.
    ///
    /// Used to detect toolchain upgrades while the language server is running.
    toolchain_signature: RwLock<Option<String>>,
}

impl Default for MetalCompiler {
    fn default() -> Self {
        Self::new()
    }
}

impl MetalCompiler {
    /// Create a new `MetalCompiler`.
    ///
    /// A unique temporary directory is created under the system temp dir
    /// to hold intermediate compilation files.
    pub fn new() -> Self {
        let temp_dir = std::env::temp_dir().join(format!("metal-analyzer-{}", std::process::id()));
        if let Err(e) = std::fs::create_dir_all(&temp_dir) {
            warn!("Failed to create temp directory {:?}: {}", temp_dir, e);
        }

        let diagnostic_re =
            Regex::new(r"^(.*?):(\d+):(\d+):\s*(error|warning|note):\s*(.*)$").unwrap();

        Self {
            temp_dir,
            diagnostic_re,
            system_include_paths: RwLock::new(Vec::new()),
            extra_include_paths: RwLock::new(Vec::new()),
            extra_flags: RwLock::new(Vec::new()),
            platform: RwLock::new(CompilerPlatform::Auto),
            include_discovery_lock: tokio::sync::Mutex::new(()),
            toolchain_signature: RwLock::new(None),
        }
    }

    /// Run `xcrun metal -v` to parse default search paths from stderr.
    /// This allows us to resolve `<metal_stdlib>` and other system headers.
    pub async fn discover_system_includes(&self) {
        let detected_signature = Self::detect_toolchain_signature().await;
        self.discover_system_includes_with_signature(detected_signature)
            .await;
    }

    async fn discover_system_includes_with_signature(&self, detected_signature: Option<String>) {
        let mut command = xcrun_command();
        let output = match command
            .args(["metal", "-v", "-E", "-"]) // -E to preprocess, - to read from stdin
            .stdin(std::process::Stdio::null())
            .output()
            .await
        {
            Ok(o) => o,
            Err(e) => {
                warn!("Failed to run xcrun metal -v: {e}");
                if let Some(signature) = detected_signature {
                    self.store_toolchain_signature(Some(signature));
                }
                return;
            }
        };

        // Different Xcode / toolchain versions can print include search details to
        // either stderr or stdout, so we parse both streams.
        let discovery_output = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        let mut paths = parse_include_search_paths(&discovery_output);
        if paths.is_empty() {
            paths = fallback_include_paths_from_toolchain_signature(detected_signature.as_deref());
        }

        if paths.is_empty() {
            warn!("No system include paths found in `metal -v` output or fallback heuristics");
        } else {
            debug!("Discovered system include paths: {:?}", paths);
        }

        if let Ok(mut guard) = self.system_include_paths.write() {
            *guard = paths;
        }
        self.store_toolchain_signature(detected_signature);
    }

    /// Ensure system include paths are available before compiling.
    pub async fn ensure_system_includes_ready(&self) {
        let detected_signature = Self::detect_toolchain_signature().await;
        if self.include_cache_is_fresh(detected_signature.as_deref()) {
            return;
        }

        let _guard = self.include_discovery_lock.lock().await;
        let detected_signature = Self::detect_toolchain_signature().await;
        if self.include_cache_is_fresh(detected_signature.as_deref()) {
            return;
        }
        self.discover_system_includes_with_signature(detected_signature)
            .await;
    }

    // ── Configuration ────────────────────────────────────────────────────

    /// Register additional include search paths (e.g. from workspace roots).
    ///
    /// These are passed as `-I <path>` to the compiler and help resolve
    /// relative `#include` directives in projects with deep directory trees.
    pub fn add_include_paths(&self, paths: impl IntoIterator<Item = PathBuf>) {
        if let Ok(mut guard) = self.extra_include_paths.write() {
            guard.extend(paths);
        }
    }

    /// Return a snapshot of the currently registered extra include paths.
    pub fn get_include_paths(&self) -> Vec<PathBuf> {
        self.extra_include_paths
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Return the discovered system include paths.
    pub fn get_system_include_paths(&self) -> Vec<PathBuf> {
        self.system_include_paths
            .read()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    fn include_cache_is_fresh(&self, detected_signature: Option<&str>) -> bool {
        let include_paths = self.get_system_include_paths();
        if include_paths.is_empty() {
            return false;
        }

        // If all discovered paths disappeared, likely because the toolchain was
        // upgraded/replaced while the server is still alive.
        if include_paths.iter().all(|path| !path.exists()) {
            return false;
        }

        let Some(detected_signature) = detected_signature else {
            return true;
        };
        self.cached_toolchain_signature()
            .is_some_and(|cached_signature| cached_signature == detected_signature)
    }

    fn cached_toolchain_signature(&self) -> Option<String> {
        self.toolchain_signature
            .read()
            .map(|signature| signature.clone())
            .unwrap_or(None)
    }

    fn store_toolchain_signature(&self, signature: Option<String>) {
        if let Ok(mut guard) = self.toolchain_signature.write() {
            *guard = signature;
        }
    }

    async fn detect_toolchain_signature() -> Option<String> {
        let mut command = xcrun_command();
        let output = command.args(["--find", "metal"]).output().await.ok()?;
        if !output.status.success() {
            return None;
        }
        let raw_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if raw_path.is_empty() {
            return None;
        }
        let path = PathBuf::from(raw_path);
        Some(
            path.canonicalize()
                .unwrap_or(path)
                .display()
                .to_string(),
        )
    }

    /// Register additional compiler flags (e.g. `-std=metal4.0`, `-DFOO=1`).
    #[allow(dead_code)]
    pub fn add_flags(&self, flags: impl IntoIterator<Item = String>) {
        if let Ok(mut guard) = self.extra_flags.write() {
            guard.extend(flags);
        }
    }

    /// Replace all extra include paths with the given set.
    #[allow(dead_code)]
    pub fn set_include_paths(&self, paths: Vec<PathBuf>) {
        if let Ok(mut guard) = self.extra_include_paths.write() {
            *guard = paths;
        }
    }

    /// Replace all extra flags with the given set.
    #[allow(dead_code)]
    pub fn set_flags(&self, flags: Vec<String>) {
        if let Ok(mut guard) = self.extra_flags.write() {
            *guard = flags;
        }
    }

    /// Configure how diagnostics compilation should infer Metal platform macros.
    pub(crate) fn set_platform(&self, platform: CompilerPlatform) {
        if let Ok(mut guard) = self.platform.write() {
            *guard = platform;
        }
    }

    /// Register workspace root folders as include search paths.
    ///
    /// For each root we add:
    /// - the root itself
    /// - any immediate child directories (one level of nesting)
    ///
    /// This makes it easy to resolve includes from anywhere inside a
    /// monorepo-style project without requiring per-file configuration.
    pub fn add_workspace_roots(&self, roots: &[PathBuf]) {
        let mut paths = Vec::new();
        for root in roots {
            if root.is_dir() {
                paths.push(root.to_path_buf());
                // Also add immediate subdirectories – many projects put
                // their kernel/shader source trees one level down.
                if let Ok(entries) = std::fs::read_dir(root) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_dir() {
                            paths.push(p);
                        }
                    }
                }
            }
        }
        self.add_include_paths(paths);
    }

    // ── Compilation ──────────────────────────────────────────────────────

    /// Compile the given Metal source text and return parsed diagnostics.
    ///
    /// * `source` – the full text of the Metal shader file.
    /// * `uri` – the original document URI (used to derive include paths).
    pub async fn compile(&self, source: &str, uri: &str) -> Vec<MetalDiagnostic> {
        let mut include_paths = Self::include_paths_from_uri(uri);
        include_paths.extend(self.get_system_include_paths().iter().map(|p| p.display().to_string()));
        self.compile_with_include_paths(source, uri, &include_paths).await
    }

    /// Compile with explicit include paths provided by the server.
    pub async fn compile_with_include_paths(
        &self,
        source: &str,
        uri: &str,
        include_paths: &[String],
    ) -> Vec<MetalDiagnostic> {
        // Always place temp artifacts under the process temp directory.
        // This avoids creating sibling `.lsp-*` files next to user sources.
        let compilation_id = NEXT_COMPILATION_ID.fetch_add(1, Ordering::Relaxed);

        if let Err(e) = tokio::fs::create_dir_all(&self.temp_dir).await {
            error!("Failed to create compiler temp dir {:?}: {}", self.temp_dir, e);
            return vec![MetalDiagnostic {
                file: uri.strip_prefix("file://").map(|s| s.replace("%20", " ")),
                line: 0,
                column: 0,
                severity: DiagnosticSeverity::ERROR,
                message: format!("Failed to create temporary directory: {e}"),
            }];
        }
        let temp_file = self.temp_dir.join(format!("shader-{compilation_id}.metal"));
        let air_file = self.temp_dir.join(format!("shader-{compilation_id}.air"));

        if let Err(e) = tokio::fs::write(&temp_file, source).await {
            error!("Failed to write temporary shader file: {}", e);
            return vec![MetalDiagnostic {
                file: uri.strip_prefix("file://").map(|s| s.replace("%20", " ")),
                line: 0,
                column: 0,
                severity: DiagnosticSeverity::ERROR,
                message: format!("Failed to write temporary file: {e}"),
            }];
        }

        let mut args = vec![
            "metal".to_string(),
            "-c".to_string(),
            temp_file.display().to_string(),
            "-o".to_string(),
            air_file.display().to_string(),
            "-fno-color-diagnostics".to_string(),
        ];

        let merged_include_paths = self.collect_include_paths(uri, include_paths);
        for p in &merged_include_paths {
            args.push("-I".to_string());
            args.push(p.clone());
        }

        // ── Effective flags ──────────────────────────────────────────────
        let (platform, effective_flags) = self.resolve_effective_flags();
        debug!(
            "Resolved compiler flags (platform={}): {:?}",
            platform.as_setting_value(),
            effective_flags
        );
        args.extend(effective_flags);

        debug!("Running: xcrun {}", args.join(" "));

        let mut command = xcrun_command();
        let result = command.args(&args).output().await;

        let _ = tokio::fs::remove_file(&temp_file).await;
        let _ = tokio::fs::remove_file(&air_file).await;

        match result {
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                debug!("Metal compiler stderr:\n{}", stderr);
                let original_path = uri.strip_prefix("file://").map(|s| s.replace("%20", " "));
                self.parse_diagnostics(&stderr)
                    .into_iter()
                    .map(|diag| {
                        remap_diagnostic_file(
                            diag,
                            original_path.as_deref(),
                            &temp_file,
                        )
                    })
                    .collect()
            }
            Err(e) => {
                error!("Failed to run Metal compiler: {}", e);
                vec![MetalDiagnostic {
                    file: uri.strip_prefix("file://").map(|s| s.replace("%20", " ")),
                    line: 0,
                    column: 0,
                    severity: DiagnosticSeverity::ERROR,
                    message: format!("Failed to run Metal compiler: {e}"),
                }]
            }
        }
    }

    /// Check whether the Metal compiler toolchain is available on this system.
    pub async fn is_available() -> bool {
        let mut command = xcrun_command();
        command
            .args(["--find", "metal"])
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    // ── Private helpers ──────────────────────────────────────────────────

    /// Parse the compiler's stderr output into a list of diagnostics.
    fn parse_diagnostics(&self, output: &str) -> Vec<MetalDiagnostic> {
        let mut diagnostics = Vec::new();

        for line in output.lines() {
            if let Some(diag) = self.parse_diagnostic_line(line) {
                diagnostics.push(diag);
            }
        }

        diagnostics
    }

    /// Attempt to parse a single line of compiler output.
    ///
    /// Expected format: `filename:line:column: severity: message`
    fn parse_diagnostic_line(&self, line: &str) -> Option<MetalDiagnostic> {
        let caps = self.diagnostic_re.captures(line)?;

        let file = caps.get(1).map(|m| m.as_str().to_owned());
        let line_num: u32 = caps.get(2)?.as_str().parse().ok()?;
        let column: u32 = caps.get(3)?.as_str().parse().ok()?;
        let severity_str = caps.get(4)?.as_str();
        let message = caps.get(5)?.as_str().to_string();

        let severity = match severity_str {
            "error" => DiagnosticSeverity::ERROR,
            "warning" => DiagnosticSeverity::WARNING,
            "note" => DiagnosticSeverity::INFORMATION,
            _ => DiagnosticSeverity::HINT,
        };

        // Convert from 1-based (compiler) to 0-based (LSP).
        Some(MetalDiagnostic {
            file,
            line: line_num.saturating_sub(1),
            column: column.saturating_sub(1),
            severity,
            message,
        })
    }

    /// Derive include search paths from a `file://` URI.
    ///
    /// Unlike a simple parent+grandparent approach, this walks up the
    /// **entire** ancestor chain of the source file's directory all the way
    /// to the filesystem root. This ensures that deeply nested relative
    /// includes such as `#include "../../../common/utils.h"` (common in
    /// deeply nested shader projects) resolve correctly.
    ///
    /// A `BTreeSet` is used to collect unique paths and the result is
    /// returned in a deterministic (sorted) order.
    fn include_paths_from_uri(uri: &str) -> Vec<String> {
        let path_str = if let Some(stripped) = uri.strip_prefix("file://") {
            // Percent-decode the path (basic: spaces only; full decoding could
            // be added later).
            stripped.replace("%20", " ")
        } else {
            return Vec::new();
        };

        let path = Path::new(&path_str);
        let mut unique = BTreeSet::new();

        // Walk up from the file's own directory through every ancestor.
        if let Some(start) = path.parent() {
            let mut current = start;
            loop {
                unique.insert(current.to_string_lossy().into_owned());
                match current.parent() {
                    Some(parent) if parent != current => current = parent,
                    _ => break,
                }
            }
        }

        // Convert to Vec<String> with the most specific (deepest) paths first
        // so the compiler prefers closer directories.
        let mut result: Vec<String> = unique.into_iter().collect();
        result.reverse();
        result
    }

    fn collect_include_paths(&self, _uri: &str, include_paths: &[String]) -> Vec<String> {
        let mut merged = BTreeSet::new();

        // Callers already provide ancestor paths (via compute_include_paths or
        // include_paths_from_uri) and system paths, so we only need to merge
        // with extra_include_paths registered via configuration.
        for p in include_paths {
            merged.insert(p.clone());
        }
        if let Ok(guard) = self.extra_include_paths.read() {
            for p in guard.iter() {
                merged.insert(p.display().to_string());
            }
        }

        merged.into_iter().collect()
    }

    fn resolve_effective_flags(&self) -> (CompilerPlatform, Vec<String>) {
        let user_flags = self
            .extra_flags
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        let platform = self
            .platform
            .read()
            .map(|guard| *guard)
            .unwrap_or_default();

        (platform, Self::build_effective_flags(&user_flags, platform))
    }

    fn build_effective_flags(user_flags: &[String], platform: CompilerPlatform) -> Vec<String> {
        let mut effective_flags = user_flags.to_vec();
        if Self::flags_define_platform_context(user_flags) {
            return effective_flags;
        }

        if let Some(default_define) = platform.default_injected_define() {
            effective_flags.push(default_define.to_string());
        }

        effective_flags
    }

    fn flags_define_platform_context(flags: &[String]) -> bool {
        let mut iter = flags.iter().peekable();
        while let Some(flag) = iter.next() {
            if Self::is_platform_define_flag(flag) || Self::is_target_or_sdk_flag(flag) {
                return true;
            }

            if flag.trim().eq_ignore_ascii_case("-D")
                && iter
                    .peek()
                    .is_some_and(|next_flag| Self::is_platform_macro_name(next_flag))
            {
                return true;
            }
        }

        false
    }

    fn is_platform_define_flag(flag: &str) -> bool {
        let trimmed = flag.trim();
        if let Some(define_body) = trimmed
            .strip_prefix("-D")
            .or_else(|| trimmed.strip_prefix("-d"))
        {
            return Self::is_platform_macro_name(define_body);
        }

        false
    }

    fn is_platform_macro_name(raw_value: &str) -> bool {
        let macro_name = raw_value
            .trim()
            .split_once('=')
            .map_or(raw_value.trim(), |(name, _)| name.trim());
        macro_name.eq_ignore_ascii_case("__METAL_MACOS__")
            || macro_name.eq_ignore_ascii_case("__METAL_IOS__")
    }

    fn is_target_or_sdk_flag(flag: &str) -> bool {
        let normalized = flag.trim().to_ascii_lowercase();
        if matches!(
            normalized.as_str(),
            "-target" | "--target" | "-isysroot" | "-sdk"
        ) {
            return true;
        }

        normalized.starts_with("-target=")
            || normalized.starts_with("--target=")
            || normalized.starts_with("-mtargetos=")
            || normalized.starts_with("-isysroot=")
            || normalized.starts_with("-sdk=")
    }
}

fn remap_diagnostic_file(
    mut diagnostic: MetalDiagnostic,
    original_path: Option<&str>,
    temp_file: &Path,
) -> MetalDiagnostic {
    let Some(raw_file) = diagnostic.file.clone() else {
        return diagnostic;
    };
    let diag_path = Path::new(&raw_file);
    let temp_matches = diag_path == temp_file
        || diag_path.canonicalize().ok() == temp_file.canonicalize().ok();
    if temp_matches {
        if let Some(original) = original_path {
            diagnostic.file = Some(original.to_owned());
        }
    } else if diag_path.is_relative()
        && let Some(original) = original_path
        && let Some(parent) = Path::new(original).parent()
    {
        let resolved = parent.join(diag_path);
        diagnostic.file = Some(
            resolved
                .canonicalize()
                .unwrap_or(resolved)
                .display()
                .to_string(),
        );
    } else {
        diagnostic.file = Some(
            diag_path
                .canonicalize()
                .unwrap_or_else(|_| diag_path.to_path_buf())
                .display()
                .to_string(),
        );
    }
    diagnostic
}

impl Drop for MetalCompiler {
    fn drop(&mut self) {
        // Best-effort cleanup of the temporary directory.
        let _ = std::fs::remove_dir_all(&self.temp_dir);
    }
}

#[cfg(test)]
#[path = "../../tests/src/metal/compiler_tests.rs"]
mod tests;
