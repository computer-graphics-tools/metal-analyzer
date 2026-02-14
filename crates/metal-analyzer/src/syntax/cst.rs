use rowan::Language;

use crate::syntax::kind::SyntaxKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MetalLanguage {}

impl Language for MetalLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        let raw = raw.0;
        assert!(raw <= SyntaxKind::AttributeArgList as u16);
        // SAFETY: The assertion ensures that the value is within the range of valid discriminants
        // for SyntaxKind, which is repr(u16).
        unsafe { std::mem::transmute(raw) }
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<MetalLanguage>;
pub type SyntaxToken = rowan::SyntaxToken<MetalLanguage>;
pub type SyntaxElement = rowan::SyntaxElement<MetalLanguage>;
