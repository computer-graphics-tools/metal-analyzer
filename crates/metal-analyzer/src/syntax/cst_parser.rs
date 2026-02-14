use rowan::{GreenNode, GreenNodeBuilder};

use crate::syntax::{kind::SyntaxKind, lexer::Lexer};

pub struct Parser<'a> {
    tokens: Vec<(SyntaxKind, &'a str)>,
    pos: usize,
    builder: GreenNodeBuilder<'static>,
    line_start: bool,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        let tokens: Vec<_> = Lexer::new(input).collect();
        Self {
            tokens,
            pos: 0,
            builder: GreenNodeBuilder::new(),
            line_start: true,
        }
    }

    pub fn parse(mut self) -> GreenNode {
        self.start_node(SyntaxKind::Root);
        self.parse_root();
        self.finish_node();
        self.builder.finish()
    }

    fn start_node(
        &mut self,
        kind: SyntaxKind,
    ) {
        self.builder.start_node(kind.into());
    }

    fn finish_node(&mut self) {
        self.builder.finish_node();
    }

    fn parse_root(&mut self) {
        while !self.is_eof() {
            if self.line_start && self.at(SyntaxKind::Hash) {
                self.parse_preprocessor();
                continue;
            }
            self.skip_trivia();
            if self.is_eof() {
                break;
            }

            match self.peek() {
                SyntaxKind::KwKernel
                | SyntaxKind::KwVertex
                | SyntaxKind::KwFragment
                | SyntaxKind::KwMesh
                | SyntaxKind::KwObject => {
                    self.parse_function_def();
                },
                SyntaxKind::KwStruct => {
                    self.parse_struct_def();
                },
                SyntaxKind::KwClass => {
                    self.parse_class_def();
                },
                SyntaxKind::KwEnum => {
                    self.parse_enum_def();
                },
                SyntaxKind::KwNamespace => {
                    self.parse_namespace_def();
                },
                SyntaxKind::KwTemplate => {
                    self.parse_template_def();
                },
                SyntaxKind::KwTypedef => {
                    self.parse_typedef_def();
                },
                SyntaxKind::KwUsing => {
                    self.parse_using_def();
                },
                _ => {
                    if !self.parse_function_or_variable_def() {
                        // Consume unexpected token to make progress
                        self.bump();
                    }
                },
            }
        }
    }

    fn parse_preprocessor(&mut self) {
        let kind = match self.peek_nth_non_trivia(1) {
            Some(SyntaxKind::Ident) => {
                let directive = self.peek_text_nth_non_trivia(1).unwrap_or_default();
                match directive {
                    "include" => SyntaxKind::PreprocInclude,
                    "define" => SyntaxKind::PreprocDefine,
                    "if" => SyntaxKind::PreprocIf,
                    "ifdef" => SyntaxKind::PreprocIfdef,
                    "ifndef" => SyntaxKind::PreprocIfndef,
                    "elif" => SyntaxKind::PreprocElif,
                    "else" => SyntaxKind::PreprocElse,
                    "endif" => SyntaxKind::PreprocEndif,
                    "pragma" => SyntaxKind::PreprocPragma,
                    _ => SyntaxKind::PreprocDefine,
                }
            },
            _ => SyntaxKind::PreprocDefine,
        };

        self.start_node(kind);
        self.consume_until_newline();
        self.finish_node();
    }

    fn parse_function_def(&mut self) {
        self.start_node(SyntaxKind::FunctionDef);
        // Attribute (kernel/vertex/fragment)
        self.bump();
        self.skip_trivia();

        // Return type
        self.parse_type_ref();
        self.skip_trivia();

        // Name
        if self.at(SyntaxKind::Ident) {
            self.bump();
        }
        self.skip_trivia();

        // Parameters
        if self.at(SyntaxKind::LParen) {
            self.parse_parameter_list();
        }
        self.skip_trivia();

        // Body
        if self.at(SyntaxKind::LBrace) {
            self.parse_block();
        }

        self.finish_node();
    }

    fn parse_function_or_variable_def(&mut self) -> bool {
        if !self.looks_like_declaration() {
            return false;
        }

        if self.looks_like_function() {
            self.start_node(SyntaxKind::FunctionDef);
            self.parse_type_ref();
            self.skip_trivia();
            if self.at(SyntaxKind::Ident) {
                self.bump();
            }
            self.skip_trivia();
            if self.at(SyntaxKind::LParen) {
                self.bump();
            }
            self.parse_parameter_list();
            self.skip_trivia();
            if self.at(SyntaxKind::LBrace) {
                self.parse_block();
            } else if self.at(SyntaxKind::Semicolon) {
                self.bump();
            }
            self.finish_node();
            return true;
        }

        self.start_node(SyntaxKind::VariableDef);
        self.parse_type_ref();
        self.skip_trivia();
        if self.at(SyntaxKind::Ident) {
            self.bump();
        }
        self.skip_trivia();
        if self.at(SyntaxKind::LDoubleBracket) {
            self.parse_attribute();
        }
        while !self.is_eof() && !self.at(SyntaxKind::Semicolon) && !self.at(SyntaxKind::LBrace) {
            self.bump();
        }
        if self.at(SyntaxKind::Semicolon) {
            self.bump();
        }
        self.finish_node();
        true
    }

    fn parse_struct_def(&mut self) {
        self.start_node(SyntaxKind::StructDef);
        self.bump(); // struct keyword
        self.skip_trivia();

        if self.at(SyntaxKind::Ident) {
            self.bump();
        }
        self.skip_trivia();

        if self.at(SyntaxKind::LBrace) {
            self.start_node(SyntaxKind::Block);
            self.bump();
            while !self.is_eof() && !self.at(SyntaxKind::RBrace) {
                self.skip_trivia();
                if self.is_eof() || self.at(SyntaxKind::RBrace) {
                    break;
                }
                self.parse_field_def();
            }
            if self.at(SyntaxKind::RBrace) {
                self.bump();
            }
            self.finish_node();
        }
        self.skip_trivia();

        if self.at(SyntaxKind::Semicolon) {
            self.bump();
        }

        self.finish_node();
    }

    fn parse_class_def(&mut self) {
        self.start_node(SyntaxKind::ClassDef);
        self.bump(); // class keyword
        self.skip_trivia();
        if self.at(SyntaxKind::Ident) {
            self.bump();
        }
        self.skip_trivia();
        if self.at(SyntaxKind::LBrace) {
            self.parse_block_like();
        }
        if self.at(SyntaxKind::Semicolon) {
            self.bump();
        }
        self.finish_node();
    }

    fn parse_enum_def(&mut self) {
        self.start_node(SyntaxKind::EnumDef);
        self.bump(); // enum keyword
        self.skip_trivia();
        if self.at(SyntaxKind::KwClass) {
            self.bump();
            self.skip_trivia();
        }
        if self.at(SyntaxKind::Ident) {
            self.bump();
        }
        self.skip_trivia();
        if self.at(SyntaxKind::LBrace) {
            self.parse_block_like();
        }
        if self.at(SyntaxKind::Semicolon) {
            self.bump();
        }
        self.finish_node();
    }

    fn parse_namespace_def(&mut self) {
        self.start_node(SyntaxKind::NamespaceDef);
        self.bump(); // namespace keyword
        self.skip_trivia();
        if self.at(SyntaxKind::Ident) {
            self.bump();
        }
        self.skip_trivia();
        if self.at(SyntaxKind::LBrace) {
            self.parse_block_like();
        }
        self.finish_node();
    }

    fn parse_template_def(&mut self) {
        self.start_node(SyntaxKind::TemplateDef);
        self.bump(); // template
        self.skip_trivia();

        if self.at(SyntaxKind::Less) {
            self.bump(); // consume '<'

            // Parse template parameters
            while !self.at(SyntaxKind::Greater) && !self.is_eof() {
                self.skip_trivia();
                self.start_node(SyntaxKind::TemplateParameter);

                // Handle typename or class keyword
                if self.at(SyntaxKind::KwTypename) || self.at(SyntaxKind::KwClass) {
                    self.bump();
                } else {
                    self.parse_type_ref();
                }

                self.skip_trivia();

                // Parse parameter name
                if self.at(SyntaxKind::Ident) {
                    self.bump();
                }

                self.skip_trivia();

                // Handle default value
                if self.at(SyntaxKind::Equal) {
                    self.bump();
                    // consume until comma or greater
                    while !self.at(SyntaxKind::Comma) && !self.at(SyntaxKind::Greater) && !self.is_eof() {
                        if self.at(SyntaxKind::Less) {
                            self.consume_balanced(SyntaxKind::Less, SyntaxKind::Greater);
                        } else if self.at(SyntaxKind::LParen) {
                            self.consume_balanced(SyntaxKind::LParen, SyntaxKind::RParen);
                        } else {
                            self.bump();
                        }
                    }
                }

                self.finish_node(); // TemplateParameter

                self.skip_trivia();

                // Handle comma between parameters
                if self.at(SyntaxKind::Comma) {
                    self.bump();
                }
            }

            if self.at(SyntaxKind::Greater) {
                self.bump(); // consume '>'
            }
        }

        self.skip_trivia();
        self.finish_node();
    }

    fn parse_typedef_def(&mut self) {
        self.start_node(SyntaxKind::TypedefDef);
        self.bump(); // typedef
        self.skip_trivia();
        self.parse_type_ref();
        self.skip_trivia();
        if self.at(SyntaxKind::Ident) {
            self.bump();
        }
        self.skip_trivia();
        if self.at(SyntaxKind::Semicolon) {
            self.bump();
        }
        self.finish_node();
    }

    fn parse_using_def(&mut self) {
        self.start_node(SyntaxKind::UsingDef);
        self.bump(); // using
        self.skip_trivia();
        while !self.is_eof() && !self.at(SyntaxKind::Semicolon) {
            self.bump();
        }
        if self.at(SyntaxKind::Semicolon) {
            self.bump();
        }
        self.finish_node();
    }

    fn parse_field_def(&mut self) {
        self.start_node(SyntaxKind::FieldDef);
        let start_pos = self.pos;

        self.parse_type_ref();
        self.skip_trivia();

        if self.at(SyntaxKind::Ident) {
            self.bump();
        }
        self.skip_trivia();

        while self.at(SyntaxKind::LDoubleBracket) {
            self.parse_attribute();
            self.skip_trivia();
        }

        // Consume any remaining declarator tokens until the terminator.
        while !self.is_eof() && !self.at(SyntaxKind::Semicolon) && !self.at(SyntaxKind::RBrace) {
            self.bump();
        }
        if self.at(SyntaxKind::Semicolon) {
            self.bump();
        }

        // Ensure progress to avoid infinite loops on unexpected syntax.
        if self.pos == start_pos {
            self.bump();
        }
        self.finish_node();
    }

    fn parse_parameter_list(&mut self) {
        self.start_node(SyntaxKind::ParameterList);
        if self.at(SyntaxKind::LParen) {
            self.bump();
        }

        while !self.is_eof() && !self.at(SyntaxKind::RParen) {
            self.skip_trivia();
            if self.is_eof() || self.at(SyntaxKind::RParen) {
                break;
            }

            let pos_before = self.pos;
            self.parse_parameter();

            // Ensure progress
            if self.pos == pos_before {
                self.bump();
            }

            self.skip_trivia();
            if self.at(SyntaxKind::Comma) {
                self.bump();
            }
        }

        if self.at(SyntaxKind::RParen) {
            self.bump();
        }
        self.finish_node();
    }

    fn parse_parameter(&mut self) {
        self.start_node(SyntaxKind::Parameter);
        self.parse_type_ref();
        self.skip_trivia();
        if self.at(SyntaxKind::Ident) {
            self.bump();
        }
        self.skip_trivia();
        if self.at(SyntaxKind::LDoubleBracket) {
            self.parse_attribute();
        }
        self.finish_node();
    }

    fn parse_attribute(&mut self) {
        self.start_node(SyntaxKind::Attribute);
        self.bump(); // [[
        while !self.is_eof() && !self.at(SyntaxKind::RDoubleBracket) {
            self.bump();
        }
        if self.at(SyntaxKind::RDoubleBracket) {
            self.bump();
        }
        self.finish_node();
    }

    fn parse_block(&mut self) {
        self.start_node(SyntaxKind::Block);
        self.bump(); // LBrace

        while !self.is_eof() && !self.at(SyntaxKind::RBrace) {
            self.skip_trivia();
            if self.is_eof() || self.at(SyntaxKind::RBrace) {
                break;
            }
            self.parse_statement();
        }

        if self.at(SyntaxKind::RBrace) {
            self.bump();
        }
        self.finish_node();
    }

    fn parse_block_like(&mut self) {
        self.start_node(SyntaxKind::Block);
        if self.at(SyntaxKind::LBrace) {
            self.bump();
        }
        while !self.is_eof() && !self.at(SyntaxKind::RBrace) {
            self.skip_trivia();
            if self.is_eof() || self.at(SyntaxKind::RBrace) {
                break;
            }
            self.parse_statement();
        }
        if self.at(SyntaxKind::RBrace) {
            self.bump();
        }
        self.finish_node();
    }

    fn parse_statement(&mut self) {
        match self.peek() {
            SyntaxKind::LBrace => self.parse_block(),
            SyntaxKind::KwReturn => {
                self.start_node(SyntaxKind::ReturnStmt);
                self.bump();
                self.skip_trivia();
                if !self.at(SyntaxKind::Semicolon) {
                    self.parse_expression();
                }
                if self.at(SyntaxKind::Semicolon) {
                    self.bump();
                }
                self.finish_node();
            },
            SyntaxKind::KwIf => {
                self.start_node(SyntaxKind::IfStmt);
                self.bump();
                self.skip_trivia();
                if self.at(SyntaxKind::LParen) {
                    self.consume_balanced(SyntaxKind::LParen, SyntaxKind::RParen);
                }
                self.skip_trivia();
                self.parse_statement();
                self.skip_trivia();
                if self.at(SyntaxKind::KwElse) {
                    self.bump();
                    self.skip_trivia();
                    self.parse_statement();
                }
                self.finish_node();
            },
            SyntaxKind::KwFor => {
                self.start_node(SyntaxKind::ForStmt);
                self.bump();
                self.skip_trivia();
                if self.at(SyntaxKind::LParen) {
                    self.bump();
                    self.skip_trivia();
                    self.parse_declaration_or_expression_until(SyntaxKind::Semicolon);
                    if self.at(SyntaxKind::Semicolon) {
                        self.bump();
                    }
                    self.skip_trivia();
                    if !self.at(SyntaxKind::Semicolon) {
                        self.parse_expression();
                    }
                    if self.at(SyntaxKind::Semicolon) {
                        self.bump();
                    }
                    self.skip_trivia();
                    if !self.at(SyntaxKind::RParen) {
                        self.parse_expression();
                    }
                    if self.at(SyntaxKind::RParen) {
                        self.bump();
                    }
                }
                self.skip_trivia();
                self.parse_statement();
                self.finish_node();
            },
            SyntaxKind::KwWhile => {
                self.start_node(SyntaxKind::WhileStmt);
                self.bump();
                self.skip_trivia();
                if self.at(SyntaxKind::LParen) {
                    self.consume_balanced(SyntaxKind::LParen, SyntaxKind::RParen);
                }
                self.skip_trivia();
                self.parse_statement();
                self.finish_node();
            },
            SyntaxKind::KwSwitch => {
                self.start_node(SyntaxKind::SwitchStmt);
                self.bump();
                self.skip_trivia();
                if self.at(SyntaxKind::LParen) {
                    self.consume_balanced(SyntaxKind::LParen, SyntaxKind::RParen);
                }
                self.skip_trivia();
                self.parse_statement();
                self.finish_node();
            },
            SyntaxKind::KwCase => {
                self.start_node(SyntaxKind::CaseStmt);
                self.bump();
                self.skip_trivia();
                if !self.at(SyntaxKind::Colon) {
                    self.parse_expression();
                }
                if self.at(SyntaxKind::Colon) {
                    self.bump();
                }
                self.finish_node();
            },
            SyntaxKind::KwBreak => {
                self.start_node(SyntaxKind::BreakStmt);
                self.bump();
                if self.at(SyntaxKind::Semicolon) {
                    self.bump();
                }
                self.finish_node();
            },
            SyntaxKind::KwContinue => {
                self.start_node(SyntaxKind::ContinueStmt);
                self.bump();
                if self.at(SyntaxKind::Semicolon) {
                    self.bump();
                }
                self.finish_node();
            },
            _ => {
                self.start_node(SyntaxKind::ExprStmt);
                if !self.parse_declaration_or_expression_until(SyntaxKind::Semicolon) {
                    self.parse_expression();
                }
                if self.at(SyntaxKind::Semicolon) {
                    self.bump();
                }
                self.finish_node();
            },
        }
    }

    fn parse_declaration_or_expression_until(
        &mut self,
        end: SyntaxKind,
    ) -> bool {
        if !self.looks_like_declaration() {
            return false;
        }
        self.start_node(SyntaxKind::DeclStmt);
        self.parse_type_ref();
        self.skip_trivia();
        if self.at(SyntaxKind::Ident) {
            self.bump();
        }
        while !self.is_eof() && !self.at(end) {
            self.bump();
        }
        self.finish_node();
        true
    }

    fn parse_type_ref(&mut self) -> bool {
        if !self.at_type_starter() {
            return false;
        }

        let mut seen_core = false;

        self.start_node(SyntaxKind::TypeRef);
        while !self.is_eof() {
            match self.peek() {
                SyntaxKind::Ident => {
                    let Some(next) = self.peek_nth_non_trivia(1) else {
                        // EOF or trivia only — treat as the (incomplete) type name.
                        self.bump();
                        break;
                    };

                    match next {
                        SyntaxKind::DoubleColon | SyntaxKind::Less | SyntaxKind::Star | SyntaxKind::Amp => {
                            // Qualified name / template / pointer / reference.
                            seen_core = true;
                            self.bump();
                        },
                        SyntaxKind::Ident => {
                            // Ident Ident ... is usually "Type Name", but we also
                            // see "Mod Type Name" in some codebases (macros).
                            if matches!(self.peek_nth_non_trivia(2), Some(SyntaxKind::Ident)) {
                                // Mod Type Name — consume Mod and continue.
                                self.bump();
                            } else {
                                // Type Name — consume Type and stop before Name.
                                self.bump();
                                break;
                            }
                        },
                        SyntaxKind::KwConst | SyntaxKind::KwVolatile => {
                            // Post-type qualifiers: `MyType const x`.
                            seen_core = true;
                            self.bump();
                        },
                        other if self.is_type_keyword(other) => {
                            // Likely a macro/modifier before a real type keyword,
                            // e.g. `METAL_FUNC void f(...)`.
                            self.bump();
                        },
                        _ => {
                            // If we've already seen the core type (e.g. `void`),
                            // this identifier is almost certainly the declarator
                            // name, so stop without consuming it.
                            if seen_core {
                                break;
                            }

                            // Otherwise, treat it as the core type name.
                            self.bump();
                            break;
                        },
                    }
                },
                SyntaxKind::KwVoid
                | SyntaxKind::KwBool
                | SyntaxKind::KwChar
                | SyntaxKind::KwChar8
                | SyntaxKind::KwChar16
                | SyntaxKind::KwChar32
                | SyntaxKind::KwWchar
                | SyntaxKind::KwShort
                | SyntaxKind::KwInt
                | SyntaxKind::KwLong
                | SyntaxKind::KwFloat
                | SyntaxKind::KwDouble
                | SyntaxKind::KwSigned
                | SyntaxKind::KwUnsigned
                | SyntaxKind::KwAuto
                | SyntaxKind::KwHalf
                | SyntaxKind::KwBFloat
                | SyntaxKind::KwBFloat16 => {
                    seen_core = true;
                    self.bump();
                },
                SyntaxKind::KwConst
                | SyntaxKind::KwVolatile
                | SyntaxKind::KwStatic
                | SyntaxKind::KwExtern
                | SyntaxKind::KwMutable
                | SyntaxKind::KwTypename
                | SyntaxKind::KwThread
                | SyntaxKind::KwThreadgroup
                | SyntaxKind::KwDevice
                | SyntaxKind::KwConstant
                | SyntaxKind::DoubleColon => {
                    self.bump();
                },
                SyntaxKind::Whitespace | SyntaxKind::Comment => {
                    // Stop before trailing trivia that separates the type from the declarator name,
                    // e.g. `void main` or `float a`.
                    if seen_core && let Some(SyntaxKind::Ident) = self.peek_nth_non_trivia(0) {
                        let after_ident = self.peek_nth_non_trivia(1);
                        if !matches!(
                            after_ident,
                            Some(SyntaxKind::DoubleColon | SyntaxKind::Less | SyntaxKind::Star | SyntaxKind::Amp)
                        ) {
                            break;
                        }
                    }
                    self.bump();
                },
                SyntaxKind::Less => {
                    self.consume_balanced(SyntaxKind::Less, SyntaxKind::Greater);
                },
                SyntaxKind::Star | SyntaxKind::Amp => {
                    self.bump();
                },
                _ => break,
            }
        }
        self.finish_node();
        true
    }

    fn parse_expression(&mut self) {
        self.parse_binary_expression(0);
    }

    fn parse_binary_expression(
        &mut self,
        min_prec: u8,
    ) {
        self.parse_unary_expression();
        while let Some((prec, right_assoc)) = self.binary_precedence(self.peek()) {
            if prec < min_prec {
                break;
            }
            let next_min = if right_assoc {
                prec
            } else {
                prec + 1
            };
            self.start_node(SyntaxKind::BinaryExpr);
            self.bump();
            self.parse_binary_expression(next_min);
            self.finish_node();
        }
    }

    fn parse_unary_expression(&mut self) {
        if self.is_unary_operator(self.peek()) {
            self.start_node(SyntaxKind::UnaryExpr);
            self.bump();
            self.parse_unary_expression();
            self.finish_node();
            return;
        }
        self.parse_postfix_expression();
    }

    fn parse_postfix_expression(&mut self) {
        self.parse_primary_expression();
        loop {
            match self.peek() {
                SyntaxKind::LParen => {
                    self.start_node(SyntaxKind::CallExpr);
                    self.consume_balanced(SyntaxKind::LParen, SyntaxKind::RParen);
                    self.finish_node();
                },
                SyntaxKind::LBracket => {
                    self.start_node(SyntaxKind::IndexExpr);
                    self.consume_balanced(SyntaxKind::LBracket, SyntaxKind::RBracket);
                    self.finish_node();
                },
                SyntaxKind::Dot | SyntaxKind::Arrow => {
                    self.start_node(SyntaxKind::MemberExpr);
                    self.bump();
                    if self.at(SyntaxKind::Ident) {
                        self.bump();
                    }
                    self.finish_node();
                },
                SyntaxKind::PlusPlus | SyntaxKind::MinusMinus => {
                    self.start_node(SyntaxKind::PostfixExpr);
                    self.bump();
                    self.finish_node();
                },
                _ => break,
            }
        }
    }

    fn parse_primary_expression(&mut self) {
        match self.peek() {
            SyntaxKind::Integer
            | SyntaxKind::Float
            | SyntaxKind::String
            | SyntaxKind::Char
            | SyntaxKind::RawString
            | SyntaxKind::KwTrue
            | SyntaxKind::KwFalse
            | SyntaxKind::KwNullptr => {
                self.start_node(SyntaxKind::LiteralExpr);
                self.bump();
                self.finish_node();
            },
            SyntaxKind::Ident => {
                self.start_node(SyntaxKind::LiteralExpr);
                self.bump();
                self.finish_node();
            },
            SyntaxKind::LParen => {
                self.start_node(SyntaxKind::CastExpr);
                self.consume_balanced(SyntaxKind::LParen, SyntaxKind::RParen);
                self.finish_node();
            },
            _ => {
                self.bump();
            },
        }
    }

    fn skip_trivia(&mut self) {
        while !self.is_eof() {
            match self.peek() {
                SyntaxKind::Whitespace | SyntaxKind::Comment => {
                    self.bump();
                },
                _ => break,
            }
        }
    }

    fn peek(&self) -> SyntaxKind {
        if self.is_eof() {
            return SyntaxKind::Error;
        }
        self.tokens[self.pos].0
    }

    fn at(
        &self,
        kind: SyntaxKind,
    ) -> bool {
        self.peek() == kind
    }

    fn bump(&mut self) {
        if !self.is_eof() {
            let (kind, text) = self.tokens[self.pos];
            self.builder.token(kind.into(), text);
            self.pos += 1;
            if kind == SyntaxKind::Whitespace && text.contains('\n') {
                self.line_start = true;
            } else if kind != SyntaxKind::Comment && kind != SyntaxKind::Whitespace {
                self.line_start = false;
            }
        }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn at_type_starter(&self) -> bool {
        matches!(
            self.peek(),
            SyntaxKind::Ident
                | SyntaxKind::KwVoid
                | SyntaxKind::KwBool
                | SyntaxKind::KwChar
                | SyntaxKind::KwChar8
                | SyntaxKind::KwChar16
                | SyntaxKind::KwChar32
                | SyntaxKind::KwWchar
                | SyntaxKind::KwShort
                | SyntaxKind::KwInt
                | SyntaxKind::KwLong
                | SyntaxKind::KwFloat
                | SyntaxKind::KwDouble
                | SyntaxKind::KwSigned
                | SyntaxKind::KwUnsigned
                | SyntaxKind::KwConst
                | SyntaxKind::KwVolatile
                | SyntaxKind::KwAuto
                | SyntaxKind::KwStatic
                | SyntaxKind::KwExtern
                | SyntaxKind::KwMutable
                | SyntaxKind::KwTypename
                | SyntaxKind::KwThread
                | SyntaxKind::KwThreadgroup
                | SyntaxKind::KwDevice
                | SyntaxKind::KwConstant
                | SyntaxKind::KwHalf
                | SyntaxKind::KwBFloat
                | SyntaxKind::KwBFloat16
        )
    }

    fn binary_precedence(
        &self,
        kind: SyntaxKind,
    ) -> Option<(u8, bool)> {
        match kind {
            SyntaxKind::Equal
            | SyntaxKind::PlusEqual
            | SyntaxKind::MinusEqual
            | SyntaxKind::StarEqual
            | SyntaxKind::SlashEqual
            | SyntaxKind::PercentEqual
            | SyntaxKind::CaretEqual
            | SyntaxKind::AmpEqual
            | SyntaxKind::PipeEqual
            | SyntaxKind::LeftShiftEqual
            | SyntaxKind::RightShiftEqual => Some((1, true)),
            SyntaxKind::OrOr => Some((2, false)),
            SyntaxKind::AndAnd => Some((3, false)),
            SyntaxKind::Pipe => Some((4, false)),
            SyntaxKind::Caret => Some((5, false)),
            SyntaxKind::Amp => Some((6, false)),
            SyntaxKind::EqualEqual | SyntaxKind::NotEqual => Some((7, false)),
            SyntaxKind::Less | SyntaxKind::Greater | SyntaxKind::LessEqual | SyntaxKind::GreaterEqual => {
                Some((8, false))
            },
            SyntaxKind::LeftShift | SyntaxKind::RightShift => Some((9, false)),
            SyntaxKind::Plus | SyntaxKind::Minus => Some((10, false)),
            SyntaxKind::Star | SyntaxKind::Slash | SyntaxKind::Percent => Some((11, false)),
            _ => None,
        }
    }

    fn is_unary_operator(
        &self,
        kind: SyntaxKind,
    ) -> bool {
        matches!(
            kind,
            SyntaxKind::Plus
                | SyntaxKind::Minus
                | SyntaxKind::Star
                | SyntaxKind::Amp
                | SyntaxKind::Exclaim
                | SyntaxKind::Tilde
                | SyntaxKind::PlusPlus
                | SyntaxKind::MinusMinus
                | SyntaxKind::KwSizeof
                | SyntaxKind::KwAlignof
                | SyntaxKind::KwNew
                | SyntaxKind::KwDelete
        )
    }

    fn consume_balanced(
        &mut self,
        open: SyntaxKind,
        close: SyntaxKind,
    ) {
        if !self.at(open) {
            return;
        }
        let mut depth = 0usize;
        while !self.is_eof() {
            if self.at(open) {
                depth += 1;
            } else if self.at(close) {
                depth = depth.saturating_sub(1);
                self.bump();
                if depth == 0 {
                    break;
                }
                continue;
            }
            self.bump();
        }
    }

    fn consume_until_newline(&mut self) {
        while !self.is_eof() {
            let (kind, text) = self.tokens[self.pos];
            self.bump();
            if kind == SyntaxKind::Whitespace && text.contains('\n') {
                break;
            }
        }
    }

    fn peek_nth_non_trivia(
        &self,
        nth: usize,
    ) -> Option<SyntaxKind> {
        let mut count = 0usize;
        let mut idx = self.pos;
        while idx < self.tokens.len() {
            let (kind, _) = self.tokens[idx];
            if !matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Comment) {
                if count == nth {
                    return Some(kind);
                }
                count += 1;
            }
            idx += 1;
        }
        None
    }

    fn peek_text_nth_non_trivia(
        &self,
        nth: usize,
    ) -> Option<&'a str> {
        let mut count = 0usize;
        let mut idx = self.pos;
        while idx < self.tokens.len() {
            let (kind, text) = self.tokens[idx];
            if !matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Comment) {
                if count == nth {
                    return Some(text);
                }
                count += 1;
            }
            idx += 1;
        }
        None
    }

    fn looks_like_declaration(&self) -> bool {
        let mut idx = self.pos;
        let mut saw_type_keyword = false;
        let mut first_ident = false;
        let mut second_ident = false;

        while idx < self.tokens.len() {
            let (kind, _) = self.tokens[idx];
            if matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Comment) {
                idx += 1;
                continue;
            }

            if self.is_type_keyword(kind) {
                saw_type_keyword = true;
                idx += 1;
                continue;
            }

            if kind == SyntaxKind::Ident {
                if !first_ident {
                    first_ident = true;
                    idx += 1;
                    continue;
                }
                second_ident = true;
                break;
            }

            if kind == SyntaxKind::DoubleColon || kind == SyntaxKind::Less {
                saw_type_keyword = true;
                idx += 1;
                continue;
            }

            break;
        }

        if saw_type_keyword && first_ident {
            return true;
        }

        first_ident && second_ident
    }

    fn looks_like_function(&self) -> bool {
        let mut idx = self.pos;
        let mut lparen_idx = None;

        while idx < self.tokens.len() {
            let (kind, _) = self.tokens[idx];
            if matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Comment) {
                idx += 1;
                continue;
            }
            if kind == SyntaxKind::LParen {
                lparen_idx = Some(idx);
                break;
            }
            if kind == SyntaxKind::Semicolon || kind == SyntaxKind::LBrace {
                break;
            }
            idx += 1;
        }

        let Some(lparen_idx) = lparen_idx else {
            return false;
        };

        let name_idx =
            (0..lparen_idx).rev().find(|i| !matches!(self.tokens[*i].0, SyntaxKind::Whitespace | SyntaxKind::Comment));

        let Some(name_idx) = name_idx else {
            return false;
        };
        if self.tokens[name_idx].0 != SyntaxKind::Ident {
            return false;
        }

        (0..name_idx).any(|i| {
            let kind = self.tokens[i].0;
            self.is_type_keyword(kind) || kind == SyntaxKind::Ident
        })
    }

    fn is_type_keyword(
        &self,
        kind: SyntaxKind,
    ) -> bool {
        matches!(
            kind,
            SyntaxKind::KwVoid
                | SyntaxKind::KwBool
                | SyntaxKind::KwChar
                | SyntaxKind::KwChar8
                | SyntaxKind::KwChar16
                | SyntaxKind::KwChar32
                | SyntaxKind::KwWchar
                | SyntaxKind::KwShort
                | SyntaxKind::KwInt
                | SyntaxKind::KwLong
                | SyntaxKind::KwFloat
                | SyntaxKind::KwDouble
                | SyntaxKind::KwSigned
                | SyntaxKind::KwUnsigned
                | SyntaxKind::KwConst
                | SyntaxKind::KwVolatile
                | SyntaxKind::KwAuto
                | SyntaxKind::KwStatic
                | SyntaxKind::KwExtern
                | SyntaxKind::KwMutable
                | SyntaxKind::KwTypename
                | SyntaxKind::KwThread
                | SyntaxKind::KwThreadgroup
                | SyntaxKind::KwDevice
                | SyntaxKind::KwConstant
                | SyntaxKind::KwHalf
                | SyntaxKind::KwBFloat
                | SyntaxKind::KwBFloat16
        )
    }
}

#[cfg(test)]
#[path = "../../tests/src/syntax/cst_parser_tests.rs"]
mod tests;
