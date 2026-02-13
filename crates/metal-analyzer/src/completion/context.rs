use tower_lsp::lsp_types::Position;

use crate::syntax::{cst::SyntaxNode, helpers, kind::SyntaxKind};

/// Describes the syntactic context at the cursor position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CursorContext {
    /// Inside an attribute bracket `[[ … ]]`.
    Attribute,
    /// After a dot (member access / swizzle).
    MemberAccess {
        receiver: String,
    },
    /// After `#` (preprocessor directive).
    Preprocessor,
    /// Inside an `#include` directive.
    Include,
    /// General / top-level context.
    General,
}

pub(crate) fn detect_context(
    text: &str,
    position: Position,
    root: Option<SyntaxNode>,
) -> CursorContext {
    if let Some(root) = root
        && let Some(ctx) = detect_context_from_tree(&root, text, position)
    {
        return ctx;
    }

    detect_context_from_text(text, position)
}

fn detect_context_from_tree(
    root: &SyntaxNode,
    source: &str,
    position: Position,
) -> Option<CursorContext> {
    let node = helpers::node_at_position(root, source, position)?;

    let mut current = node;
    loop {
        match current.kind() {
            SyntaxKind::Attribute => return Some(CursorContext::Attribute),
            SyntaxKind::PreprocInclude => return Some(CursorContext::Include),
            SyntaxKind::PreprocDefine
            | SyntaxKind::PreprocIf
            | SyntaxKind::PreprocIfdef
            | SyntaxKind::PreprocIfndef
            | SyntaxKind::PreprocElif
            | SyntaxKind::PreprocElse
            | SyntaxKind::PreprocEndif
            | SyntaxKind::PreprocPragma => return Some(CursorContext::Preprocessor),
            SyntaxKind::MemberExpr => {
                let receiver = helpers::node_text(&current, source).to_string();
                return Some(CursorContext::MemberAccess {
                    receiver,
                });
            },
            _ => {},
        }
        current = current.parent()?;
    }
}

fn detect_context_from_text(
    text: &str,
    position: Position,
) -> CursorContext {
    let line_idx = position.line as usize;
    let char_idx = position.character as usize;

    let line = match text.lines().nth(line_idx) {
        Some(l) => l,
        None => return CursorContext::General,
    };

    let prefix: String = line.chars().take(char_idx).collect();
    let trimmed = prefix.trim_start();

    if trimmed.starts_with("#include") {
        return CursorContext::Include;
    }

    if trimmed.starts_with('#') {
        return CursorContext::Preprocessor;
    }

    let open_count = prefix.matches("[[").count();
    let close_count = prefix.matches("]]").count();
    if open_count > close_count {
        return CursorContext::Attribute;
    }

    let trimmed_end = prefix.trim_end();
    if let Some(before_dot) = trimmed_end.strip_suffix('.') {
        let receiver: String = before_dot
            .chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if !receiver.is_empty() {
            return CursorContext::MemberAccess {
                receiver,
            };
        }
    }

    CursorContext::General
}
