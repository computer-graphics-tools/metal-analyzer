use tower_lsp::lsp_types::SemanticTokenType;

use crate::{
    semantic_tokens::LEGEND_TYPES,
    syntax::{cst::SyntaxToken, kind::SyntaxKind},
};

pub(crate) fn map_ast_kind_to_token_type(kind: &str) -> Option<SemanticTokenType> {
    match kind {
        "NamespaceDecl" => Some(SemanticTokenType::NAMESPACE),
        "CXXRecordDecl" | "ClassTemplateSpecializationDecl" | "ClassTemplateDecl" => Some(SemanticTokenType::STRUCT),
        "EnumDecl" => Some(SemanticTokenType::ENUM),
        "EnumConstantDecl" => Some(SemanticTokenType::ENUM_MEMBER),
        "FunctionDecl" | "FunctionTemplateDecl" | "CXXMethodDecl" => Some(SemanticTokenType::FUNCTION),
        "VarDecl" | "FieldDecl" | "ParmVarDecl" => Some(SemanticTokenType::VARIABLE),
        "TypedefDecl" | "TypeAliasDecl" => Some(SemanticTokenType::TYPE),
        "TemplateTypeParmDecl" => Some(SemanticTokenType::TYPE_PARAMETER),
        "NonTypeTemplateParmDecl" => Some(SemanticTokenType::VARIABLE),
        _ => None,
    }
}

pub(crate) fn get_token_type_index(token_type: SemanticTokenType) -> u32 {
    LEGEND_TYPES.iter().position(|t| *t == token_type).unwrap_or(0) as u32
}

pub(crate) fn token_is_type_name(token: &SyntaxToken) -> bool {
    token.parent_ancestors().any(|node| node.kind() == SyntaxKind::TypeRef)
}
