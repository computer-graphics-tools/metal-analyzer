# Metal Analyzer for Zed and VS Code

A fast, robust Language Server Protocol implementation for Apple's **Metal Shading Language**, written in Rust.

## Prerequisites

- **macOS** (required for the Metal compiler toolchain)
- **Rust** (to build from source)
- **Zed** or **VS Code**
- **Bun** (only for building the VS Code extension)

## Installation

### 1. Install the Language Server

Both editor extensions now auto-install `metal-analyzer` on macOS by downloading the latest signed GitHub Release asset when `metal-analyzer` is not already in your PATH.

If you prefer a manual global install, install the `metal-analyzer` binary using Cargo:

```bash
cargo install --path crates/metal-analyzer
```

Ensure that `~/.cargo/bin` is in your PATH. Verify installation by running:

```bash
metal-analyzer --version
```

### 2. Install the Zed Extension

1. Open **Zed**.
2. Press `Cmd+Shift+P` to open the command palette.
3. Type and select **"zed: install dev extension"**.
4. Navigate to the `editors/zed` folder in this repository and select it.

### 3. Install the VS Code Extension

You can install the extension locally from this repository as a `.vsix` package.

1. Build the extension package:

```bash
cd editors/vscode
bun install
bun run compile
bun run package
```

1. Install the generated `.vsix` in VS Code:
   - Open **VS Code**
   - Press `Cmd+Shift+P`
   - Run **Extensions: Install from VSIX...**
   - Select the generated `.vsix` file in `editors/vscode`

1. Configure the server binary path (optional):
   - Open Settings
   - Search for `metal-analyzer.serverPath`
   - Set it to either:
     - `metal-analyzer` (default; uses PATH first, then auto-installs on macOS), or
     - an absolute path like `~/.cargo/bin/metal-analyzer`

1. Configure language server behavior (optional):
   - Search for the `metal-analyzer.` prefix in VS Code settings.
   - Available groups:
   - `metal-analyzer.formatting.*` (formatter command/options; style is always read from `.clang-format`)
   - `metal-analyzer.diagnostics.*` (on-type/on-save/debounce/scope)
   - `metal-analyzer.indexing.*` (background indexing controls, including `excludePaths` for folder/file scan exclusions)
   - `metal-analyzer.compiler.*` (extra include paths, platform context, and compiler flags)
   - `metal-analyzer.logging.level` (error/warn/info/debug/trace)

1. Restart VS Code and open a `.metal` file.

Quick verification:

- Run `metal-analyzer --version` in terminal.
- In a `.metal` file, hover or go-to-definition on a known symbol.

Troubleshooting:

- If packaging fails due dependency resolution issues, reinstall deps and re-run packaging:
  - `rm -rf editors/vscode/node_modules`
  - `cd editors/vscode && bun install && bun run package`
- If `.metal` files show `C/C++` diagnostics in VS Code, remove conflicting associations such as `"files.associations": { "*.metal": "cpp" }` and keep language mode set to `Metal`. See `editors/vscode/README.md` for diagnostics source guidance.

## Features

- **Real-time Diagnostics**: Validates shaders with `xcrun metal` on save.
- **Auto-completion**: Support for built-in types, functions, and keywords.
- **Hover Documentation**: Documentation for built-in symbols.

## License

This project is licensed under [MIT license](LICENSE).
