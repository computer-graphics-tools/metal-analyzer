use super::*;
use crate::syntax::SyntaxTree;

fn navigation_word(
    source: &str,
    position: Position,
) -> Option<String> {
    let snapshot = SyntaxTree::parse(source);
    let root = snapshot.root();
    navigation_word_at_position(&root, source, position)
}

#[test]
fn navigation_word_falls_back_on_pointer_star() {
    let source = "const constant MyParams* params;";
    let position = Position::new(0, 23);
    assert_eq!(navigation_word(source, position).as_deref(), Some("MyParams"));
}

#[test]
fn navigation_word_falls_back_on_reference_ampersand() {
    let source = "const Foo& ref = value;";
    let position = Position::new(0, 9);
    assert_eq!(navigation_word(source, position).as_deref(), Some("Foo"));
}

#[test]
fn navigation_word_falls_back_on_rvalue_reference() {
    let source = "const Foo&& ref = value;";
    let position = Position::new(0, 9);
    assert_eq!(navigation_word(source, position).as_deref(), Some("Foo"));
}
