# Configuration

All settings are available under the `metal-analyzer.` prefix in VS Code settings.

## Server Path

- `metal-analyzer.serverPath` — Path to the metal-analyzer binary.
  The default value (`metal-analyzer`) uses PATH first, then auto-downloads
  the latest macOS release binary.

<!-- $generated-start - generated from config.rs via schema_fields() -->

## Formatting

- `metal-analyzer.formatting.enable` - Enable LSP-backed document formatting.
- `metal-analyzer.formatting.command` - Formatting executable used by metal-analyzer.
- `metal-analyzer.formatting.args` - Additional arguments passed to the formatting command.

## Diagnostics

- `metal-analyzer.diagnostics.onType` - Run diagnostics while typing.
- `metal-analyzer.diagnostics.onSave` - Run diagnostics when a document is saved.
- `metal-analyzer.diagnostics.debounceMs` - Debounce delay for on-type diagnostics and background indexing work.
- `metal-analyzer.diagnostics.scope` - Diagnostics scope. `openFiles` analyzes documents as they are opened/edited/saved. `workspace` also analyzes all `.metal` files in the workspace at startup and when settings change.

## Indexing

- `metal-analyzer.indexing.enable` - Enable background workspace indexing.
- `metal-analyzer.indexing.concurrency` - Maximum number of concurrent background indexing jobs.
- `metal-analyzer.indexing.maxFileSizeKb` - Skip workspace files larger than this size during background indexing.
- `metal-analyzer.indexing.projectGraphDepth` - Maximum include-graph traversal depth for scoped cross-file go-to-definition fallback.
- `metal-analyzer.indexing.projectGraphMaxNodes` - Maximum number of graph nodes considered during scoped cross-file go-to-definition fallback.
- `metal-analyzer.indexing.excludePaths` - Workspace paths to skip during background scanning. Relative paths are resolved from each workspace root; absolute paths are also supported. Excluded folders are skipped for both indexing and workspace-scope diagnostics.

## Compiler

- `metal-analyzer.compiler.includePaths` - Extra include directories passed to the Metal compiler.
- `metal-analyzer.compiler.extraFlags` - Extra compiler flags passed to `xcrun metal`.
- `metal-analyzer.compiler.platform` - Target platform for Metal diagnostics. Determines which platform define (e.g. `__METAL_MACOS__`) is injected unless platform flags are already present in extra flags. Values: `macos`, `ios`, `tvos`, `watchos`, `xros`.

## Logging

- `metal-analyzer.logging.level` - Runtime logging verbosity for metal-analyzer.

## Thread Pool

- `metal-analyzer.threadPool.workerThreads` - Worker thread pool size. `0` uses `available_parallelism`. Requires restart.
- `metal-analyzer.threadPool.formattingThreads` - Formatting thread pool size. Requires restart.

<!-- $generated-end -->
