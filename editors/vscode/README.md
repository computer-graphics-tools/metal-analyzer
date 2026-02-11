# Metal Shading Language (VS Code)

VS Code extension for Apple's Metal Shading Language powered by `metal-analyzer`.

## What To Expect After Install

This extension activates when you open a `.metal` file.

- There is no command palette command to run manually.
- Syntax highlighting appears for `.metal` files.
- Language features (diagnostics, hover, completion, go-to-definition) are provided by `metal-analyzer`.

## Installation (VSIX)

1. Open VS Code.
2. Run `Extensions: Install from VSIX...`.
3. Select the built `.vsix` file.
4. Reload VS Code when prompted.
5. Open any `.metal` file to activate the extension.

## Server Binary

Setting: `metal-analyzer.serverPath`

- Default: `metal-analyzer`
- Behavior on macOS:
  - If `metal-analyzer` is in `PATH`, it is used.
  - Otherwise the extension downloads the latest GitHub release binary automatically.

You can also set an absolute path, for example:

- `~/.cargo/bin/metal-analyzer`

## Settings

The extension forwards `metal-analyzer.*` settings to the language server in real time.

- `metal-analyzer.formatting.*`
  - `enabled` (default `true`)
  - `command` (default `clang-format`)
  - `args` (default `[]`)
  - formatter style always comes from the nearest `.clang-format` (or `_clang-format`) file
- `metal-analyzer.diagnostics.*`
  - `onType` (default `true`)
  - `onSave` (default `true`)
  - `debounceMs` (default `500`)
  - `scope` (default `openFiles`, or `workspace` to analyze all workspace `.metal` files at startup/config changes)
- `metal-analyzer.indexing.*`
  - `enabled` (default `true`)
  - `concurrency` (default `1`)
  - `maxFileSizeKb` (default `512`)
  - `excludePaths` (default `[]`; skips matching folders/files for both background indexing and workspace-scope diagnostics)
- `metal-analyzer.compiler.*`
  - `includePaths` (default `[]`)
  - `extraFlags` (default `[]`)
  - `platform` (default `auto`; one of `auto`, `macos`, `ios`, `none`)
- `metal-analyzer.logging.level`
  - one of `error`, `warn`, `info`, `debug`, `trace` (default `info`)

Example:

```json
{
  "metal-analyzer.diagnostics.debounceMs": 300,
  "metal-analyzer.diagnostics.scope": "workspace",
  "metal-analyzer.indexing.maxFileSizeKb": 1024,
  "metal-analyzer.indexing.excludePaths": [
    "external/vendor-shaders"
  ],
  "metal-analyzer.compiler.includePaths": [
    "/path/to/includes"
  ],
  "metal-analyzer.compiler.platform": "auto",
  "metal-analyzer.compiler.extraFlags": [
    "-DMETAL_DEBUG=1"
  ]
}
```

## Quick Verification

1. Open a `.metal` file.
2. Confirm language mode shows `Metal`.
3. Hover a known symbol or trigger completion.
4. Save to trigger diagnostics.

If features are still inactive, check `View -> Output` and select `Metal Analyzer` in the output channel.

## Diagnostics Source Troubleshooting

- `metal-analyzer` diagnostics are reported with source `metal-compiler`.
- If errors show source `C/C++`, they are not coming from `metal-analyzer`.
- In VS Code settings, remove conflicting file associations such as:

```json
{
  "files.associations": {
    "*.metal": "cpp"
  }
}
```

- Ensure the current file language mode is `Metal` (status bar, lower-right).
