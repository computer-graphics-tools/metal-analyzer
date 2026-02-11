use crate::syntax::kind::{SyntaxKind, TokenKind};
use logos::Logos;

/// A lexer that wraps `logos::Lexer` to produce `SyntaxKind` tokens.
pub struct Lexer<'a> {
    inner: logos::Lexer<'a, TokenKind>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            inner: TokenKind::lexer(input),
        }
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = (SyntaxKind, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        let token_result = self.inner.next()?;
        let text = self.inner.slice();

        let kind = match token_result {
            Ok(token) => token.into(),
            Err(_) => SyntaxKind::Error,
        };

        Some((kind, text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(input: &str) -> Vec<(SyntaxKind, &str)> {
        Lexer::new(input).collect()
    }

    #[test]
    fn test_keywords() {
        let input = "kernel void return";
        let tokens = lex(input);
        assert_eq!(
            tokens,
            vec![
                (SyntaxKind::KwKernel, "kernel"),
                (SyntaxKind::Whitespace, " "),
                (SyntaxKind::KwVoid, "void"),
                (SyntaxKind::Whitespace, " "),
                (SyntaxKind::KwReturn, "return"),
            ]
        );
    }

    #[test]
    fn test_punctuation() {
        let input = "{ } ( ) ;";
        let tokens = lex(input);
        assert_eq!(
            tokens,
            vec![
                (SyntaxKind::LBrace, "{"),
                (SyntaxKind::Whitespace, " "),
                (SyntaxKind::RBrace, "}"),
                (SyntaxKind::Whitespace, " "),
                (SyntaxKind::LParen, "("),
                (SyntaxKind::Whitespace, " "),
                (SyntaxKind::RParen, ")"),
                (SyntaxKind::Whitespace, " "),
                (SyntaxKind::Semicolon, ";"),
            ]
        );
    }

    #[test]
    fn test_identifiers_and_literals() {
        let input = "main 123 3.14 \"hello\"";
        let tokens = lex(input);
        assert_eq!(
            tokens,
            vec![
                (SyntaxKind::Ident, "main"),
                (SyntaxKind::Whitespace, " "),
                (SyntaxKind::Integer, "123"),
                (SyntaxKind::Whitespace, " "),
                (SyntaxKind::Float, "3.14"),
                (SyntaxKind::Whitespace, " "),
                (SyntaxKind::String, "\"hello\""),
            ]
        );
    }

    #[test]
    fn test_preprocessor_tokens() {
        let input = "#include <metal_stdlib>";
        let tokens = lex(input);
        assert_eq!(
            tokens,
            vec![
                (SyntaxKind::Hash, "#"),
                (SyntaxKind::Ident, "include"),
                (SyntaxKind::Whitespace, " "),
                (SyntaxKind::Less, "<"),
                (SyntaxKind::Ident, "metal_stdlib"),
                (SyntaxKind::Greater, ">"),
            ]
        );
    }

    #[test]
    fn test_operators() {
        let input = "a <<= 1 && b >= 2";
        let tokens = lex(input);
        assert_eq!(
            tokens,
            vec![
                (SyntaxKind::Ident, "a"),
                (SyntaxKind::Whitespace, " "),
                (SyntaxKind::LeftShiftEqual, "<<="),
                (SyntaxKind::Whitespace, " "),
                (SyntaxKind::Integer, "1"),
                (SyntaxKind::Whitespace, " "),
                (SyntaxKind::AndAnd, "&&"),
                (SyntaxKind::Whitespace, " "),
                (SyntaxKind::Ident, "b"),
                (SyntaxKind::Whitespace, " "),
                (SyntaxKind::GreaterEqual, ">="),
                (SyntaxKind::Whitespace, " "),
                (SyntaxKind::Integer, "2"),
            ]
        );
    }

    #[test]
    fn test_error() {
        let input = "#";
        let tokens = lex(input);
        assert_eq!(tokens, vec![(SyntaxKind::Hash, "#"),]);
    }
}
