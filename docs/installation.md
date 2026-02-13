# Installation

## Prerequisites

- **macOS** (required for the Metal compiler toolchain)
- **Rust** (to build from source)
- **Zed** or **VS Code**
- **Bun** (only for building the VS Code extension)

## Language Server

Both editor extensions auto-install `metal-analyzer` on macOS by
downloading the latest signed GitHub Release asset when `metal-analyzer`
is not already in your PATH.

If you prefer a manual global install:

```bash
cargo install --path crates/metal-analyzer
```

Ensure that `~/.cargo/bin` is in your PATH. Verify installation:

```bash
metal-analyzer --version
```

## Zed

1. Open **Zed**.
2. Press `Cmd+Shift+P` to open the command palette.
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
   - Press `Cmd+Shift+P`
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
