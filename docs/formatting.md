# Formatting

metal-analyzer provides LSP-backed document formatting by invoking
`clang-format` as a subprocess. Formatting style can be configured with a
project-local `metalfmt.toml` file, similar to how `rustfmt.toml` works for
Rust projects.

## How it works

When a formatting request is received, metal-analyzer resolves the style in
this order:

1. **`metalfmt.toml`** — walks up from the source file looking for a
   `metalfmt.toml`. If found, the TOML options are translated into an inline
   clang-format style string and passed via `--style={...}`.
2. **`.clang-format`** — if no `metalfmt.toml` is found, clang-format's
   built-in file discovery is used (`--style=file`), which looks for
   `.clang-format` or `_clang-format` in parent directories.
3. **No config** — if neither file is found, formatting is skipped
   (`--fallback-style=none`).

## metalfmt.toml

Place a `metalfmt.toml` at the root of your project (or any parent directory
of your Metal source files). All keys are optional; only the keys you set are
forwarded to clang-format.

### Example

```toml
based_on_style = "LLVM"
indent_width = 4
column_limit = 120
pointer_alignment = "Left"
sort_includes = false
bin_pack_parameters = false
bin_pack_arguments = false
allow_short_functions_on_a_single_line = "Empty"
max_empty_lines_to_keep = 1
cpp_standard = "c++17"
```

### Available keys

#### Base style

| Key              | Type   | Description                                                        |
|------------------|--------|--------------------------------------------------------------------|
| `based_on_style` | string | Base style preset. Common values: `LLVM`, `Google`, `Chromium`, `Mozilla`, `WebKit`. |

#### Indentation

| Key            | Type | Description                          |
|----------------|------|--------------------------------------|
| `indent_width` | int  | Number of columns for each indent level. |
| `use_tab`      | bool | `true` = tabs, `false` = spaces.     |
| `tab_width`    | int  | Width of a tab character in columns. |

#### Line length

| Key            | Type | Description                                  |
|----------------|------|----------------------------------------------|
| `column_limit` | int  | Maximum line width. `0` disables line wrapping. |

#### Braces

| Key                                     | Type   | Description                                                  |
|-----------------------------------------|--------|--------------------------------------------------------------|
| `break_before_braces`                   | string | Brace breaking style: `Attach`, `Linux`, `Stroustrup`, `Allman`, `GNU`, `Custom`, etc. |
| `brace_wrapping_after_function`         | bool   | Wrap brace after function definition (requires `Custom`).    |
| `brace_wrapping_after_struct`           | bool   | Wrap brace after struct definition (requires `Custom`).      |
| `brace_wrapping_after_enum`             | bool   | Wrap brace after enum definition (requires `Custom`).        |
| `brace_wrapping_after_control_statement`| string | Wrap after control statement: `Never`, `MultiLine`, `Always`.|

#### Spacing and alignment

| Key                      | Type   | Description                                                        |
|--------------------------|--------|--------------------------------------------------------------------|
| `space_before_parens`    | string | Space before parentheses: `Never`, `ControlStatements`, `Always`.  |
| `pointer_alignment`      | string | Pointer/reference alignment: `Left`, `Right`, `Middle`.            |
| `reference_alignment`    | string | Reference alignment (overrides `pointer_alignment` for references).|
| `align_after_open_bracket`| string | Alignment after open bracket: `Align`, `DontAlign`, `AlwaysBreak`.|
| `align_operands`         | string | Align operands of binary expressions: `DontAlign`, `Align`, `AlignAfterOperator`. |
| `align_trailing_comments`| bool   | Align trailing comments.                                           |

#### Includes

| Key              | Type   | Description                                                    |
|------------------|--------|----------------------------------------------------------------|
| `sort_includes`  | bool   | Sort `#include` directives.                                    |
| `include_blocks` | string | Grouping of include blocks: `Preserve`, `Merge`, `Regroup`.   |

#### Other

| Key                                        | Type   | Description                                                   |
|--------------------------------------------|--------|---------------------------------------------------------------|
| `allow_short_functions_on_a_single_line`   | string | `None`, `Empty`, `Inline`, `All`.                             |
| `allow_short_if_statements_on_a_single_line`| string | `Never`, `WithoutElse`, `OnlyFirstIf`, `AllIfsAndElse`.      |
| `allow_short_loops_on_a_single_line`       | bool   | Allow short loops on a single line.                           |
| `bin_pack_arguments`                        | bool   | Pack function call arguments into as few lines as possible.   |
| `bin_pack_parameters`                       | bool   | Pack function declaration parameters into as few lines as possible. |
| `cpp_standard`                              | string | C++ standard for parsing: `c++11`, `c++14`, `c++17`, `c++20`, `Latest`. |
| `max_empty_lines_to_keep`                  | int    | Maximum number of consecutive empty lines to keep.            |

## Editor settings

Formatting is controlled by two LSP settings (see
[configuration](configuration.md)):

- `metal-analyzer.formatting.enable` — enable or disable formatting.
- `metal-analyzer.formatting.command` — override the formatter executable
  (default: `clang-format`).
- `metal-analyzer.formatting.args` — extra arguments passed before the
  generated arguments.
