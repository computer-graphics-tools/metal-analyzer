<p align="center">
  <img
    src="./assets/logo-wide.svg"
    alt="metal-analyzer logo">
</p>

metal-analyzer is a language server that provides IDE functionality for
writing Apple's Metal Shading Language programs. You can use it with
any editor that supports the [Language Server
Protocol](https://microsoft.github.io/language-server-protocol/) (VS
Code, Zed, etc).

metal-analyzer features include real-time diagnostics (via `xcrun metal`),
auto-completion for built-in types, functions, and keywords, hover
documentation, and integrated formatting (with clang-format).

## Quick Start

See [Installation](./docs/installation.md) for setup instructions for
VS Code and Zed.

## CLI

metal-analyzer can also be used as a command-line formatter:

```sh
# Format files in-place
metal-analyzer format shader.metal compute.metal

# Check formatting without modifying files (exits 1 if changes needed)
metal-analyzer format --check shader.metal

# Format from stdin
cat shader.metal | metal-analyzer format
```

Formatting style is resolved in order:

1. **`metalfmt.toml`** — walks up from the source file looking for a
   `metalfmt.toml` (see [Formatting](./docs/formatting.md)).
2. **`.clang-format`** — if no `metalfmt.toml` is found, clang-format's
   built-in file discovery is used (`.clang-format` / `_clang-format`).
3. **No config** — if neither file is found, the file is left unchanged.

See `metal-analyzer format --help` for all options.

## Configuration

See [Configuration](./docs/configuration.md) for available settings.

## License

This project is licensed under [MIT license](LICENSE).
