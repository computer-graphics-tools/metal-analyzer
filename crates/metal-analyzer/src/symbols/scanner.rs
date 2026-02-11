use tower_lsp::lsp_types::{DocumentSymbol, SymbolKind};

use crate::syntax::ast::{self, AstNode};
use crate::syntax::cst::SyntaxNode;
use crate::syntax::helpers;
use crate::syntax::kind::SyntaxKind;

pub(crate) fn build_symbols(root: &SyntaxNode, text: &str) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    for node in root.descendants() {
        if let Some(func) = ast::FunctionDef::cast(node.clone()) {
            if let Some(name) = func.name_token() {
                let range = helpers::range_to_lsp(name.text_range(), text);
                let detail = detect_function_detail(&func);
                symbols.push(DocumentSymbol {
                    name: name.text().to_string(),
                    detail: Some(detail),
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    #[allow(deprecated)]
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: None,
                });
            }
            continue;
        }

        if let Some(def) = ast::StructDef::cast(node.clone()) {
            let symbol = struct_symbol(&def, text, SymbolKind::STRUCT);
            if let Some(symbol) = symbol {
                symbols.push(symbol);
            }
            continue;
        }

        if let Some(def) = ast::ClassDef::cast(node.clone()) {
            let symbol = named_symbol(def.syntax(), text, SymbolKind::CLASS, "class");
            if let Some(symbol) = symbol {
                symbols.push(symbol);
            }
            continue;
        }

        if let Some(def) = ast::EnumDef::cast(node.clone()) {
            let symbol = enum_symbol(&def, text);
            if let Some(symbol) = symbol {
                symbols.push(symbol);
            }
            continue;
        }

        if let Some(def) = ast::TypedefDef::cast(node.clone()) {
            let symbol = named_symbol(def.syntax(), text, SymbolKind::TYPE_PARAMETER, "typedef");
            if let Some(symbol) = symbol {
                symbols.push(symbol);
            }
            continue;
        }

        if let Some(def) = ast::UsingDef::cast(node.clone()) {
            let symbol = named_symbol(def.syntax(), text, SymbolKind::TYPE_PARAMETER, "using");
            if let Some(symbol) = symbol {
                symbols.push(symbol);
            }
            continue;
        }

        if let Some(def) = ast::TemplateParameter::cast(node.clone())
            && let Some(name) = def.name_token() {
                let range = helpers::range_to_lsp(name.text_range(), text);
                symbols.push(DocumentSymbol {
                    name: name.text().to_string(),
                    detail: Some("template param".to_string()),
                    kind: SymbolKind::TYPE_PARAMETER,
                    tags: None,
                    #[allow(deprecated)]
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: None,
                });
            }

        if let Some(def) = ast::PreprocDefine::cast(node.clone()) {
            let symbol = named_symbol(def.syntax(), text, SymbolKind::CONSTANT, "macro");
            if let Some(symbol) = symbol {
                symbols.push(symbol);
            }
            continue;
        }

        if let Some(def) = ast::VariableDef::cast(node.clone()) {
            let symbol = named_symbol(def.syntax(), text, SymbolKind::VARIABLE, "variable");
            if let Some(symbol) = symbol {
                symbols.push(symbol);
            }
        }
    }

    symbols.sort_by_key(|s| (s.range.start.line, s.range.start.character));
    symbols
}

fn struct_symbol(def: &ast::StructDef, text: &str, kind: SymbolKind) -> Option<DocumentSymbol> {
    let name = def.name_token()?;
    let range = helpers::range_to_lsp(name.text_range(), text);
    let mut children = Vec::new();
    for field in def.fields() {
        if let Some(field_name) = field.name_token() {
            let field_range = helpers::range_to_lsp(field_name.text_range(), text);
            children.push(DocumentSymbol {
                name: field_name.text().to_string(),
                detail: None,
                kind: SymbolKind::FIELD,
                tags: None,
                #[allow(deprecated)]
                deprecated: None,
                range: field_range,
                selection_range: field_range,
                children: None,
            });
        }
    }

    Some(DocumentSymbol {
        name: name.text().to_string(),
        detail: Some(format!("struct {}", name.text())),
        kind,
        tags: None,
        #[allow(deprecated)]
        deprecated: None,
        range,
        selection_range: range,
        children: if children.is_empty() {
            None
        } else {
            Some(children)
        },
    })
}

fn enum_symbol(def: &ast::EnumDef, text: &str) -> Option<DocumentSymbol> {
    let name = def.name_token()?;
    let range = helpers::range_to_lsp(name.text_range(), text);
    let mut children = Vec::new();
    if let Some(block) = def
        .syntax()
        .children()
        .find(|n| n.kind() == SyntaxKind::Block)
    {
        for token in block
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
        {
            if token.kind() == SyntaxKind::Ident {
                let tok_range = helpers::range_to_lsp(token.text_range(), text);
                children.push(DocumentSymbol {
                    name: token.text().to_string(),
                    detail: None,
                    kind: SymbolKind::ENUM_MEMBER,
                    tags: None,
                    #[allow(deprecated)]
                    deprecated: None,
                    range: tok_range,
                    selection_range: tok_range,
                    children: None,
                });
            }
        }
    }

    Some(DocumentSymbol {
        name: name.text().to_string(),
        detail: Some(format!("enum {}", name.text())),
        kind: SymbolKind::ENUM,
        tags: None,
        #[allow(deprecated)]
        deprecated: None,
        range,
        selection_range: range,
        children: if children.is_empty() {
            None
        } else {
            Some(children)
        },
    })
}

fn named_symbol(
    syntax: &SyntaxNode,
    text: &str,
    kind: SymbolKind,
    prefix: &str,
) -> Option<DocumentSymbol> {
    let name = syntax
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == SyntaxKind::Ident)?;
    let range = helpers::range_to_lsp(name.text_range(), text);
    Some(DocumentSymbol {
        name: name.text().to_string(),
        detail: Some(format!("{prefix} {}", name.text())),
        kind,
        tags: None,
        #[allow(deprecated)]
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    })
}

fn detect_function_detail(func: &ast::FunctionDef) -> String {
    let qualifier = func
        .syntax()
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .find_map(|token| match token.kind() {
            SyntaxKind::KwKernel => Some("kernel"),
            SyntaxKind::KwVertex => Some("vertex"),
            SyntaxKind::KwFragment => Some("fragment"),
            SyntaxKind::KwMesh => Some("mesh"),
            SyntaxKind::KwObject => Some("object"),
            _ => None,
        });

    let name = func
        .name_token()
        .map(|t| t.text().to_string())
        .unwrap_or_default();

    match qualifier {
        Some(q) => format!("{q} ... {name}(...)"),
        None => format!("{name}(...)"),
    }
}

/// Flatten nested DocumentSymbols into a single list (for scan_file indexing).
pub(crate) fn flatten_symbols(symbols: &[DocumentSymbol]) -> Vec<&DocumentSymbol> {
    let mut result = Vec::new();
    for sym in symbols {
        result.push(sym);
        if let Some(children) = &sym.children {
            result.extend(flatten_symbols(children));
        }
    }
    result
}
