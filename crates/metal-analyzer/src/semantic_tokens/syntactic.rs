use tower_lsp::lsp_types::SemanticTokenType;

use crate::syntax::SyntaxTree;
use crate::syntax::ast::{self, AstNode};
use crate::syntax::queries::{TokenClass, classify_token};

use super::mapping::token_is_type_name;
use super::{LineIndex, RawToken};

pub(crate) fn syntactic_tokens(snapshot: &SyntaxTree) -> Vec<RawToken> {
    let mut tokens = Vec::new();
    let source = snapshot.source();
    let line_index = LineIndex::new(source);
    let root = snapshot.root();

    for element in root.descendants_with_tokens() {
        let token = match element.into_token() {
            Some(tok) => tok,
            None => continue,
        };
        let token_type = if let Some(class) = classify_token(&token) {
            match class {
                TokenClass::Comment => SemanticTokenType::COMMENT,
                TokenClass::String => SemanticTokenType::STRING,
                TokenClass::Number => SemanticTokenType::NUMBER,
                TokenClass::Type => SemanticTokenType::TYPE,
                TokenClass::Function => SemanticTokenType::FUNCTION,
                TokenClass::Macro => SemanticTokenType::MACRO,
                TokenClass::Keyword => SemanticTokenType::KEYWORD,
                TokenClass::Operator => SemanticTokenType::OPERATOR,
                TokenClass::Property => SemanticTokenType::PROPERTY,
                TokenClass::MetalKeyword => SemanticTokenType::KEYWORD,
            }
        } else if token.kind() == crate::syntax::kind::SyntaxKind::Ident
            && token_is_type_name(&token)
        {
            SemanticTokenType::TYPE
        } else {
            continue;
        };

        tokens.push(raw_token_from_range(
            token.text_range(),
            &line_index,
            token_type,
        ));
    }

    for node in root.descendants() {
        if let Some(func) = ast::FunctionDef::cast(node.clone())
            && let Some(name) = func.name_token()
        {
            tokens.push(raw_token_from_range(
                name.text_range(),
                &line_index,
                SemanticTokenType::FUNCTION,
            ));
        }
        if let Some(def) = ast::StructDef::cast(node.clone())
            && let Some(name) = def.name_token()
        {
            tokens.push(raw_token_from_range(
                name.text_range(),
                &line_index,
                SemanticTokenType::STRUCT,
            ));
        }
        if let Some(def) = ast::ClassDef::cast(node.clone())
            && let Some(name) = def.name_token()
        {
            tokens.push(raw_token_from_range(
                name.text_range(),
                &line_index,
                SemanticTokenType::CLASS,
            ));
        }
        if let Some(def) = ast::EnumDef::cast(node.clone())
            && let Some(name) = def.name_token()
        {
            tokens.push(raw_token_from_range(
                name.text_range(),
                &line_index,
                SemanticTokenType::ENUM,
            ));
        }
        if let Some(def) = ast::TypedefDef::cast(node.clone())
            && let Some(name) = def
                .syntax()
                .children_with_tokens()
                .filter_map(|e| e.into_token())
                .find(|t| t.kind() == crate::syntax::kind::SyntaxKind::Ident)
        {
            tokens.push(raw_token_from_range(
                name.text_range(),
                &line_index,
                SemanticTokenType::TYPE_PARAMETER,
            ));
        }
    }

    tokens
}

pub(crate) fn raw_token_from_range(
    range: rowan::TextRange,
    line_index: &LineIndex,
    token_type: SemanticTokenType,
) -> RawToken {
    let start: usize = range.start().into();
    let end: usize = range.end().into();
    let (line, col) = line_index.line_col(start);
    RawToken {
        line,
        col,
        length: (end - start) as u32,
        token_type,
    }
}
