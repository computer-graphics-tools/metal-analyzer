    use super::*;
    use crate::syntax::cst::SyntaxNode;

    fn check(input: &str, expected_tree: &str) {
        let parser = Parser::new(input);
        let green = parser.parse();
        let node = SyntaxNode::new_root(green);
        let actual_tree = format!("{:#?}", node);

        // Normalize newlines and trim
        let actual_tree = actual_tree.trim();
        let expected_tree = expected_tree.trim();

        assert_eq!(actual_tree, expected_tree);
    }

    #[test]
    fn test_empty() {
        check("", "Root@0..0");
    }

    #[test]
    fn test_function_def() {
        check(
            "kernel void main() {}",
            r#"
Root@0..21
  FunctionDef@0..21
    KwKernel@0..6 "kernel"
    Whitespace@6..7 " "
    TypeRef@7..11
      KwVoid@7..11 "void"
    Whitespace@11..12 " "
    Ident@12..16 "main"
    ParameterList@16..18
      LParen@16..17 "("
      RParen@17..18 ")"
    Whitespace@18..19 " "
    Block@19..21
      LBrace@19..20 "{"
      RBrace@20..21 "}"
"#,
        );
    }

    #[test]
    fn test_struct_def() {
        check(
            "struct MyStruct { float a; };",
            r#"
Root@0..29
  StructDef@0..29
    KwStruct@0..6 "struct"
    Whitespace@6..7 " "
    Ident@7..15 "MyStruct"
    Whitespace@15..16 " "
    Block@16..28
      LBrace@16..17 "{"
      Whitespace@17..18 " "
      FieldDef@18..26
        TypeRef@18..23
          KwFloat@18..23 "float"
        Whitespace@23..24 " "
        Ident@24..25 "a"
        Semicolon@25..26 ";"
      Whitespace@26..27 " "
      RBrace@27..28 "}"
    Semicolon@28..29 ";"
"#,
        );
    }
