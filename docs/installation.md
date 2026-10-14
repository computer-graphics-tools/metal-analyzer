# Installation

## Prerequisites

- A Metal compiler toolchain:
  - **macOS**: Xcode (or the Xcode Command Line Tools with the Metal toolchain)
  - **Windows**: Apple's [Metal Developer Tools for Windows](https://developer.apple.com/metal/tools/)
- **Rust** (to build from source)
- **Zed** or **VS Code**
- **Bun** (only for building the VS Code extension)

## Language Server

Both editor extensions auto-install `metal-analyzer` on macOS and Windows
by downloading the latest signed GitHub Release asset when `metal-analyzer`
is not already in your PATH.

If you prefer a manual global install:

```bash
cargo install --path crates/metal-analyzer
```

Ensure that `~/.cargo/bin` (`%USERPROFILE%\.cargo\bin` on Windows) is in
your PATH. Verify installation:

```bash
metal-analyzer --version
```

## Windows

metal-analyzer runs the compiler from the Metal Developer Tools for Windows
for diagnostics, indexing, and go-to-definition. It looks for `metal.exe`
in your PATH first, then in the default install location
(`%ProgramFiles%\Metal Developer Tools\bin`). If you installed the tools
somewhere else, add that `bin` folder to your PATH.

metal-analyzer ships native x64 and ARM64 Windows builds. Apple's Metal
Developer Tools are x64-only; on Windows on ARM they run through the
built-in x64 emulation.

Formatting uses `clang-format` from your PATH. If it is not there,
metal-analyzer falls back to an [LLVM](https://github.com/llvm/llvm-project/releases)
install in `%ProgramFiles%\LLVM`, or to the copy bundled with Visual Studio's
**C++ Clang tools for Windows** component.

## Zed

1. Open **Zed**.
2. Press `Cmd+Shift+P` (`Ctrl+Shift+P` on Windows) to open the command palette.
3. Type and select **"zed: install dev extension"**.
4. Navigate to the `editors/zed` folder in this repository and select it.

## VS Code

You can install the extension locally from this repository as a `.vsix` package.

1. Build the extension package:

```bash
cd editors/code
bun install
bun run compile
bun run package
```

2. Install the generated `.vsix` in VS Code:
   - Open **VS Code**
   - Press `Cmd+Shift+P` (`Ctrl+Shift+P` on Windows)
   - Run **Extensions: Install from VSIX...**
   - Select the generated `.vsix` file in `editors/code`

3. Restart VS Code and open a `.metal` file.

## Troubleshooting

- If packaging fails due to dependency resolution issues, reinstall deps and re-run packaging:
  - `rm -rf editors/code/node_modules`
  - `cd editors/code && bun install && bun run package`
- If `.metal` files show `C/C++` diagnostics in VS Code, remove conflicting
  associations such as `"files.associations": { "*.metal": "cpp" }` and keep
  language mode set to `Metal`. See `editors/code/README.md` for diagnostics
  source guidance.
