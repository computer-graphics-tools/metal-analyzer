use crate::syntax::{cst::SyntaxToken, kind::SyntaxKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenClass {
    Comment,
    String,
    Number,
    Type,
    Function,
    Macro,
    Keyword,
    Operator,
    Property,
    MetalKeyword,
}

pub fn classify_token(token: &SyntaxToken) -> Option<TokenClass> {
    match token.kind() {
        SyntaxKind::Comment => Some(TokenClass::Comment),
        SyntaxKind::String | SyntaxKind::RawString | SyntaxKind::Char => Some(TokenClass::String),
        SyntaxKind::Integer | SyntaxKind::Float => Some(TokenClass::Number),
        kind if is_type_token(kind) => Some(TokenClass::Type),
        kind if is_metal_keyword(kind) => Some(TokenClass::MetalKeyword),
        kind if is_keyword(kind) => Some(TokenClass::Keyword),
        kind if is_operator(kind) => Some(TokenClass::Operator),
        _ => None,
    }
}

pub fn is_keyword(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::KwAlignas
            | SyntaxKind::KwAlignof
            | SyntaxKind::KwAsm
            | SyntaxKind::KwAuto
            | SyntaxKind::KwBool
            | SyntaxKind::KwBreak
            | SyntaxKind::KwCase
            | SyntaxKind::KwCatch
            | SyntaxKind::KwChar
            | SyntaxKind::KwChar8
            | SyntaxKind::KwChar16
            | SyntaxKind::KwChar32
            | SyntaxKind::KwClass
            | SyntaxKind::KwConst
            | SyntaxKind::KwConsteval
            | SyntaxKind::KwConstexpr
            | SyntaxKind::KwConstinit
            | SyntaxKind::KwContinue
            | SyntaxKind::KwDecltype
            | SyntaxKind::KwDefault
            | SyntaxKind::KwDelete
            | SyntaxKind::KwDo
            | SyntaxKind::KwDouble
            | SyntaxKind::KwDynamicCast
            | SyntaxKind::KwElse
            | SyntaxKind::KwEnum
            | SyntaxKind::KwExplicit
            | SyntaxKind::KwExport
            | SyntaxKind::KwExtern
            | SyntaxKind::KwFalse
            | SyntaxKind::KwFloat
            | SyntaxKind::KwFor
            | SyntaxKind::KwFriend
            | SyntaxKind::KwGoto
            | SyntaxKind::KwIf
            | SyntaxKind::KwInline
            | SyntaxKind::KwInt
            | SyntaxKind::KwLong
            | SyntaxKind::KwMutable
            | SyntaxKind::KwNamespace
            | SyntaxKind::KwNew
            | SyntaxKind::KwNoexcept
            | SyntaxKind::KwNullptr
            | SyntaxKind::KwOperator
            | SyntaxKind::KwPrivate
            | SyntaxKind::KwProtected
            | SyntaxKind::KwPublic
            | SyntaxKind::KwRegister
            | SyntaxKind::KwReinterpretCast
            | SyntaxKind::KwReturn
            | SyntaxKind::KwShort
            | SyntaxKind::KwSigned
            | SyntaxKind::KwSizeof
            | SyntaxKind::KwStatic
            | SyntaxKind::KwStaticAssert
            | SyntaxKind::KwStruct
            | SyntaxKind::KwSwitch
            | SyntaxKind::KwTemplate
            | SyntaxKind::KwThis
            | SyntaxKind::KwThreadLocal
            | SyntaxKind::KwThrow
            | SyntaxKind::KwTrue
            | SyntaxKind::KwTry
            | SyntaxKind::KwTypedef
            | SyntaxKind::KwTypeid
            | SyntaxKind::KwTypename
            | SyntaxKind::KwUnion
            | SyntaxKind::KwUnsigned
            | SyntaxKind::KwUsing
            | SyntaxKind::KwVirtual
            | SyntaxKind::KwVoid
            | SyntaxKind::KwVolatile
            | SyntaxKind::KwWchar
            | SyntaxKind::KwWhile
            | SyntaxKind::KwConcept
            | SyntaxKind::KwRequires
            | SyntaxKind::KwCoAwait
            | SyntaxKind::KwCoReturn
            | SyntaxKind::KwCoYield
            | SyntaxKind::KwModule
            | SyntaxKind::KwImport
    )
}

pub fn is_metal_keyword(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::KwKernel
            | SyntaxKind::KwVertex
            | SyntaxKind::KwFragment
            | SyntaxKind::KwMesh
            | SyntaxKind::KwObject
            | SyntaxKind::KwDevice
            | SyntaxKind::KwThreadgroup
            | SyntaxKind::KwConstant
            | SyntaxKind::KwThread
            | SyntaxKind::KwRayData
            | SyntaxKind::KwVisible
            | SyntaxKind::KwSampler
            | SyntaxKind::KwTexture
            | SyntaxKind::KwHalf
            | SyntaxKind::KwBFloat
            | SyntaxKind::KwBFloat16
    )
}

pub fn is_type_token(kind: SyntaxKind) -> bool {
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
            | SyntaxKind::KwHalf
            | SyntaxKind::KwBFloat
            | SyntaxKind::KwBFloat16
    )
}

pub fn is_operator(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Plus
            | SyntaxKind::Minus
            | SyntaxKind::Star
            | SyntaxKind::Slash
            | SyntaxKind::Percent
            | SyntaxKind::Caret
            | SyntaxKind::Amp
            | SyntaxKind::Pipe
            | SyntaxKind::Tilde
            | SyntaxKind::Exclaim
            | SyntaxKind::Equal
            | SyntaxKind::Less
            | SyntaxKind::Greater
            | SyntaxKind::PlusPlus
            | SyntaxKind::MinusMinus
            | SyntaxKind::PlusEqual
            | SyntaxKind::MinusEqual
            | SyntaxKind::StarEqual
            | SyntaxKind::SlashEqual
            | SyntaxKind::PercentEqual
            | SyntaxKind::CaretEqual
            | SyntaxKind::AmpEqual
            | SyntaxKind::PipeEqual
            | SyntaxKind::EqualEqual
            | SyntaxKind::NotEqual
            | SyntaxKind::LessEqual
            | SyntaxKind::GreaterEqual
            | SyntaxKind::AndAnd
            | SyntaxKind::OrOr
            | SyntaxKind::LeftShift
            | SyntaxKind::RightShift
            | SyntaxKind::LeftShiftEqual
            | SyntaxKind::RightShiftEqual
    )
}
