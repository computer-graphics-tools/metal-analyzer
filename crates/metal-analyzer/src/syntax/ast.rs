use crate::syntax::cst::{SyntaxNode, SyntaxToken};
use crate::syntax::kind::SyntaxKind;

pub trait AstNode: Sized {
    fn cast(syntax: SyntaxNode) -> Option<Self>;
    fn syntax(&self) -> &SyntaxNode;
}

fn first_ident_token(syntax: &SyntaxNode) -> Option<SyntaxToken> {
    syntax
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == SyntaxKind::Ident)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Root {
    syntax: SyntaxNode,
}

impl AstNode for Root {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::Root {
            Some(Self { syntax })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionDef {
    syntax: SyntaxNode,
}

impl AstNode for FunctionDef {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::FunctionDef {
            Some(Self { syntax })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

impl FunctionDef {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        first_ident_token(&self.syntax)
    }

    pub fn return_type(&self) -> Option<TypeRef> {
        self.syntax.children().find_map(TypeRef::cast)
    }

    pub fn parameter_list(&self) -> Option<ParameterList> {
        self.syntax.children().find_map(ParameterList::cast)
    }

    pub fn body(&self) -> Option<Block> {
        self.syntax.children().find_map(Block::cast)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructDef {
    syntax: SyntaxNode,
}

impl AstNode for StructDef {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::StructDef {
            Some(Self { syntax })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

impl StructDef {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        first_ident_token(&self.syntax)
    }

    pub fn fields(&self) -> impl Iterator<Item = FieldDef> {
        self.syntax.children().filter_map(FieldDef::cast)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClassDef {
    syntax: SyntaxNode,
}

impl AstNode for ClassDef {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::ClassDef {
            Some(Self { syntax })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

impl ClassDef {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        first_ident_token(&self.syntax)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnumDef {
    syntax: SyntaxNode,
}

impl AstNode for EnumDef {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::EnumDef {
            Some(Self { syntax })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

impl EnumDef {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        first_ident_token(&self.syntax)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NamespaceDef {
    syntax: SyntaxNode,
}

impl AstNode for NamespaceDef {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::NamespaceDef {
            Some(Self { syntax })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TemplateDef {
    syntax: SyntaxNode,
}

impl AstNode for TemplateDef {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::TemplateDef {
            Some(Self { syntax })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

impl TemplateDef {
    pub fn parameters(&self) -> impl Iterator<Item = TemplateParameter> {
        self.syntax.children().filter_map(TemplateParameter::cast)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TemplateParameter {
    syntax: SyntaxNode,
}

impl AstNode for TemplateParameter {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::TemplateParameter {
            Some(Self { syntax })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

impl TemplateParameter {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        first_ident_token(&self.syntax)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PreprocDefine {
    syntax: SyntaxNode,
}

impl AstNode for PreprocDefine {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::PreprocDefine {
            Some(Self { syntax })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

impl PreprocDefine {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        self.syntax
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .filter(|token| token.kind() == SyntaxKind::Ident)
            .find(|token| token.text() != "define")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VariableDef {
    syntax: SyntaxNode,
}

impl AstNode for VariableDef {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::VariableDef {
            Some(Self { syntax })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

impl VariableDef {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        first_ident_token(&self.syntax)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypedefDef {
    syntax: SyntaxNode,
}

impl AstNode for TypedefDef {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::TypedefDef {
            Some(Self { syntax })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UsingDef {
    syntax: SyntaxNode,
}

impl AstNode for UsingDef {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::UsingDef {
            Some(Self { syntax })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldDef {
    syntax: SyntaxNode,
}

impl AstNode for FieldDef {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::FieldDef {
            Some(Self { syntax })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

impl FieldDef {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        first_ident_token(&self.syntax)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParameterList {
    syntax: SyntaxNode,
}

impl AstNode for ParameterList {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::ParameterList {
            Some(Self { syntax })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

impl ParameterList {
    pub fn parameters(&self) -> impl Iterator<Item = Parameter> {
        self.syntax.children().filter_map(Parameter::cast)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Parameter {
    syntax: SyntaxNode,
}

impl AstNode for Parameter {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::Parameter {
            Some(Self { syntax })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

impl Parameter {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        first_ident_token(&self.syntax)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Block {
    syntax: SyntaxNode,
}

impl AstNode for Block {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::Block {
            Some(Self { syntax })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeRef {
    syntax: SyntaxNode,
}

impl AstNode for TypeRef {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::TypeRef {
            Some(Self { syntax })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Attribute {
    syntax: SyntaxNode,
}

impl AstNode for Attribute {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::Attribute {
            Some(Self { syntax })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr {
    Assign(SyntaxNode),
    Binary(SyntaxNode),
    Unary(SyntaxNode),
    Postfix(SyntaxNode),
    Member(SyntaxNode),
    Index(SyntaxNode),
    Call(SyntaxNode),
    Cast(SyntaxNode),
    Literal(SyntaxNode),
}

impl Expr {
    pub fn cast(syntax: SyntaxNode) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::AssignExpr => Some(Self::Assign(syntax)),
            SyntaxKind::BinaryExpr => Some(Self::Binary(syntax)),
            SyntaxKind::UnaryExpr => Some(Self::Unary(syntax)),
            SyntaxKind::PostfixExpr => Some(Self::Postfix(syntax)),
            SyntaxKind::MemberExpr => Some(Self::Member(syntax)),
            SyntaxKind::IndexExpr => Some(Self::Index(syntax)),
            SyntaxKind::CallExpr => Some(Self::Call(syntax)),
            SyntaxKind::CastExpr => Some(Self::Cast(syntax)),
            SyntaxKind::LiteralExpr => Some(Self::Literal(syntax)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Stmt {
    Return(SyntaxNode),
    If(SyntaxNode),
    For(SyntaxNode),
    While(SyntaxNode),
    Switch(SyntaxNode),
    Case(SyntaxNode),
    Break(SyntaxNode),
    Continue(SyntaxNode),
    Decl(SyntaxNode),
    Expr(SyntaxNode),
    Block(SyntaxNode),
}

impl Stmt {
    pub fn cast(syntax: SyntaxNode) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::ReturnStmt => Some(Self::Return(syntax)),
            SyntaxKind::IfStmt => Some(Self::If(syntax)),
            SyntaxKind::ForStmt => Some(Self::For(syntax)),
            SyntaxKind::WhileStmt => Some(Self::While(syntax)),
            SyntaxKind::SwitchStmt => Some(Self::Switch(syntax)),
            SyntaxKind::CaseStmt => Some(Self::Case(syntax)),
            SyntaxKind::BreakStmt => Some(Self::Break(syntax)),
            SyntaxKind::ContinueStmt => Some(Self::Continue(syntax)),
            SyntaxKind::DeclStmt => Some(Self::Decl(syntax)),
            SyntaxKind::ExprStmt => Some(Self::Expr(syntax)),
            SyntaxKind::Block => Some(Self::Block(syntax)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Ref(TypeRef),
}

impl Type {
    pub fn cast(syntax: SyntaxNode) -> Option<Self> {
        TypeRef::cast(syntax).map(Self::Ref)
    }
}
