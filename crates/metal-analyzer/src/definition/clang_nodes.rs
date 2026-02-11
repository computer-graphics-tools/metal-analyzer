use clang_ast::{BareSourceLocation, Id, SourceLocation};
use serde::Deserialize;

pub type Node = clang_ast::Node<Clang>;

/// Typed representation of Clang AST node kinds relevant to our indexer.
///
/// Each variant corresponds to a Clang AST node `"kind"` value.
/// The `Other` fallback efficiently skips all unrecognized node kinds.
#[derive(Deserialize)]
pub enum Clang {
    // --- Declarations ---
    FunctionDecl(DeclData),
    CXXRecordDecl(DeclData),
    CXXMethodDecl(DeclData),
    VarDecl(DeclData),
    FieldDecl(DeclData),
    ParmVarDecl(DeclData),
    TypedefDecl(DeclData),
    TypeAliasDecl(DeclData),
    EnumDecl(DeclData),
    EnumConstantDecl(DeclData),
    NamespaceDecl(DeclData),
    FunctionTemplateDecl(DeclData),
    ClassTemplateDecl(DeclData),
    ClassTemplateSpecializationDecl(DeclData),
    UsingDecl(DeclData),
    TemplateTypeParmDecl(DeclData),
    NonTypeTemplateParmDecl(DeclData),

    // --- References ---
    DeclRefExpr(RefExprData),
    MemberExpr(RefExprData),

    // --- Catch-all ---
    // The `loc` and `range` fields MUST be deserialized even for unrecognized
    // node kinds. The `clang-ast` crate tracks "current file" state across the
    // deserialization stream via `SourceLocation`; if we skip locations for
    // nodes like `ImportDecl` that set the file path, all subsequent nodes
    // inherit an empty file.
    #[allow(dead_code)]
    Other {
        #[serde(default)]
        loc: Option<SourceLocation>,
        #[serde(default)]
        range: Option<clang_ast::SourceRange>,
    },
}

/// Common data for all declaration nodes.
///
/// The `ty` field captures Clang's `type.qualType` string, which carries
/// the full type signature — e.g. `"void (float *, uint)"` for functions
/// or `"float4"` for variables.
#[derive(Deserialize, Debug)]
pub struct DeclData {
    pub name: Option<String>,
    pub loc: Option<SourceLocation>,
    #[serde(rename = "isImplicit")]
    pub is_implicit: Option<bool>,
    #[serde(rename = "isThisDeclarationADefinition")]
    pub is_this_declaration_a_definition: Option<bool>,
    #[serde(rename = "type")]
    pub ty: Option<QualType>,
}

/// Reference expression data (DeclRefExpr, MemberExpr).
#[derive(Deserialize, Debug)]
pub struct RefExprData {
    pub loc: Option<SourceLocation>,
    pub range: Option<clang_ast::SourceRange>,
    #[serde(rename = "referencedDecl")]
    pub referenced_decl: Option<ReferencedDecl>,
    #[serde(rename = "isImplicit")]
    pub is_implicit: Option<bool>,
}

/// Inline summary of a referenced declaration.
#[derive(Deserialize, Debug)]
pub struct ReferencedDecl {
    pub id: Id,
    pub kind: Option<String>,
    pub name: Option<String>,
}

/// Clang's qualified type representation.
#[derive(Deserialize, Debug)]
pub struct QualType {
    #[serde(rename = "qualType")]
    pub qual_type: Option<String>,
}

impl DeclData {
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    pub fn is_implicit(&self) -> bool {
        self.is_implicit.unwrap_or(false)
    }
    pub fn is_definition(&self) -> bool {
        self.is_this_declaration_a_definition.unwrap_or(true)
    }
    pub fn qual_type(&self) -> Option<&str> {
        self.ty.as_ref().and_then(|t| t.qual_type.as_deref())
    }
}

/// Extract the best concrete source location from a [`SourceLocation`].
///
/// Prefers the expansion location (where a macro was invoked — the position
/// the user sees in their source file) over the spelling location (inside the
/// macro definition).
pub fn resolve_loc(loc: &SourceLocation) -> Option<&BareSourceLocation> {
    loc.expansion_loc.as_ref().or(loc.spelling_loc.as_ref())
}
