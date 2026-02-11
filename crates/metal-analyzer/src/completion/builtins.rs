use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, Documentation, InsertTextFormat, MarkupContent, MarkupKind,
};

use crate::metal::builtins::{self, BuiltinEntry, BuiltinKind};

pub(crate) fn builtin_to_completion_item(
    entry: &BuiltinEntry,
    sort_prefix: &str,
) -> CompletionItem {
    let kind = match entry.kind {
        BuiltinKind::Keyword => CompletionItemKind::KEYWORD,
        BuiltinKind::Type => CompletionItemKind::CLASS,
        BuiltinKind::Function => CompletionItemKind::FUNCTION,
        BuiltinKind::Attribute => CompletionItemKind::PROPERTY,
        BuiltinKind::Snippet => CompletionItemKind::SNIPPET,
        BuiltinKind::Constant => CompletionItemKind::CONSTANT,
    };

    let insert_text_format =
        if entry.is_snippet || entry.insert_text.as_ref().is_some_and(|t| t.contains('$')) {
            Some(InsertTextFormat::SNIPPET)
        } else {
            Some(InsertTextFormat::PLAIN_TEXT)
        };

    CompletionItem {
        label: entry.label.clone(),
        kind: Some(kind),
        detail: Some(entry.detail.clone()),
        documentation: if entry.documentation.is_empty() {
            None
        } else {
            Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: entry.documentation.clone(),
            }))
        },
        insert_text: entry.insert_text.clone(),
        insert_text_format,
        sort_text: Some(format!("{}_{}", sort_prefix, entry.label)),
        ..Default::default()
    }
}

pub(crate) fn first_identifier(s: &str) -> Option<String> {
    let s = s.trim();
    let ident: String = s
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if ident.is_empty() { None } else { Some(ident) }
}

/// Try to detect a function name from a line like:
///   `vertex float4 myFunc(` or `void helper(`
pub(crate) fn detect_function_name(line: &str) -> Option<String> {
    let paren_idx = line.find('(')?;
    let before_paren = line[..paren_idx].trim();
    let name = before_paren.split_whitespace().last()?;
    if name.chars().all(|c| c.is_alphanumeric() || c == '_')
        && !name.is_empty()
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
    {
        if builtins::keywords().contains(&name) {
            return None;
        }
        Some(name.to_string())
    } else {
        None
    }
}

pub(crate) static PREPROCESSOR_DIRECTIVES: &[(&str, &str)] = &[
    ("include", "Include a header file"),
    ("define", "Define a preprocessor macro"),
    ("undef", "Undefine a preprocessor macro"),
    ("if", "Conditional compilation"),
    ("ifdef", "Conditional compilation — if defined"),
    ("ifndef", "Conditional compilation — if not defined"),
    ("elif", "Else-if conditional compilation"),
    ("else", "Else branch of conditional compilation"),
    ("endif", "End conditional compilation block"),
    ("pragma", "Compiler pragma directive"),
    ("line", "Set line number for diagnostics"),
    ("error", "Generate a compiler error"),
    ("warning", "Generate a compiler warning"),
];

pub(crate) static METAL_HEADERS: &[(&str, &str)] = &[
    (
        "metal_stdlib",
        "The main Metal standard library header. Includes math, geometric, texture, and other built-in functions.",
    ),
    (
        "metal_compute",
        "Metal compute-specific functions and types.",
    ),
    (
        "metal_graphics",
        "Metal graphics (render pipeline) specific functions and types.",
    ),
    (
        "metal_geometric",
        "Geometric functions: `normalize`, `dot`, `cross`, `distance`, `length`, `reflect`, `refract`.",
    ),
    (
        "metal_math",
        "Math functions: trigonometry, exponential, clamping, etc.",
    ),
    ("metal_matrix", "Matrix types and operations."),
    (
        "metal_pack",
        "Pack and unpack functions for compressed data formats.",
    ),
    ("metal_texture", "Texture types and sampling functions."),
    (
        "metal_atomic",
        "Atomic operations for shared memory synchronization.",
    ),
    (
        "metal_integer",
        "Integer math functions: `abs`, `clamp`, `min`, `max`, `popcount`, etc.",
    ),
    (
        "metal_relational",
        "Relational and logical functions: `all`, `any`, `isfinite`, `isinf`, `isnan`, `select`, `step`.",
    ),
    (
        "metal_simdgroup",
        "SIMD-group (warp/wavefront) functions: shuffle, reduce, prefix sum, ballot.",
    ),
    (
        "metal_common",
        "Common functions shared across Metal library components.",
    ),
    (
        "simd/simd.h",
        "SIMD types and functions shared with the CPU side.",
    ),
];

pub(crate) static TEXTURE_METHODS: &[(&str, &str, &str)] = &[
    (
        "sample",
        "T sample(sampler s, float2 coord)",
        "Sample the texture at the given coordinates using a sampler.",
    ),
    (
        "read",
        "T read(uint2 coord)",
        "Read a texel at the specified integer coordinates (no filtering).",
    ),
    (
        "write",
        "void write(T value, uint2 coord)",
        "Write a value to the texture at the specified integer coordinates.",
    ),
    (
        "get_width",
        "uint get_width(uint lod = 0)",
        "Return the width of the texture in texels at the given mip level.",
    ),
    (
        "get_height",
        "uint get_height(uint lod = 0)",
        "Return the height of the texture in texels at the given mip level.",
    ),
    (
        "get_depth",
        "uint get_depth(uint lod = 0)",
        "Return the depth of a 3-D texture at the given mip level.",
    ),
    (
        "get_num_mip_levels",
        "uint get_num_mip_levels()",
        "Return the number of mip levels in the texture.",
    ),
    (
        "get_num_samples",
        "uint get_num_samples()",
        "Return the number of samples per texel (MSAA textures).",
    ),
    (
        "sample_compare",
        "float sample_compare(sampler s, float2 coord, float compare_value)",
        "Sample a depth texture and compare against a reference value.",
    ),
    (
        "gather",
        "vec<T, 4> gather(sampler s, float2 coord, int2 offset = int2(0))",
        "Gather four texels that would be used for bilinear filtering.",
    ),
    (
        "fence",
        "void fence()",
        "Ensure all previous writes to this texture are visible to subsequent reads.",
    ),
    (
        "get_array_size",
        "uint get_array_size()",
        "Return the number of slices in a texture array.",
    ),
];
