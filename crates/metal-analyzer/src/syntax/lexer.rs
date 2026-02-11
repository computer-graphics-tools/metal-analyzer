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
#[path = "../../tests/src/syntax/lexer_tests.rs"]
mod tests;
