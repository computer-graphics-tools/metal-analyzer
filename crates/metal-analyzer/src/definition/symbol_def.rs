use serde::{Deserialize, Serialize};

/// A definition or declaration found in the AST.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolDef {
    /// Clang AST node id (e.g. `"0x714cc9008"`).
    pub id: String,
    /// Symbol name.
    pub name: String,
    /// AST node kind (e.g. `FunctionDecl`, `CXXRecordDecl`).
    pub kind: String,
    /// Absolute file path where the symbol is defined.
    pub file: String,
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number.
    pub col: u32,
    /// Whether this is a definition (true) or just a declaration (false).
    pub is_definition: bool,
    /// Normalized type name for variables/fields/parameters.
    ///
    /// Used to implement `textDocument/typeDefinition` by resolving the type
    /// declaration/definition in the same translation unit (file + includes).
    pub type_name: Option<String>,
    /// Full qualified type string from Clang (e.g. `"void (float *, uint)"`
    /// for functions, `"float4"` for variables). Used for hover display.
    pub qual_type: Option<String>,
}
