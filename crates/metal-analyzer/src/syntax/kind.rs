use logos::Logos;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
    // Tokens
    Error = 0,
    Whitespace,
    Comment,

    // Identifiers & Literals
    Ident,
    Integer,
    Float,
    String,
    Char,
    RawString,

    // Preprocessor / punctuation
    Hash,
    HashHash,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    LDoubleBracket,
    RDoubleBracket,
    Semicolon,
    Colon,
    Comma,
    Dot,
    Ellipsis,
    Arrow,
    ArrowStar,
    DotStar,
    DoubleColon,
    Question,

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    Amp,
    Pipe,
    Tilde,
    Exclaim,
    Equal,
    Less,
    Greater,
    PlusPlus,
    MinusMinus,
    PlusEqual,
    MinusEqual,
    StarEqual,
    SlashEqual,
    PercentEqual,
    CaretEqual,
    AmpEqual,
    PipeEqual,
    EqualEqual,
    NotEqual,
    LessEqual,
    GreaterEqual,
    AndAnd,
    OrOr,
    LeftShift,
    RightShift,
    LeftShiftEqual,
    RightShiftEqual,

    // C/C++ Keywords
    KwAlignas,
    KwAlignof,
    KwAsm,
    KwAuto,
    KwBool,
    KwBreak,
    KwCase,
    KwCatch,
    KwChar,
    KwChar8,
    KwChar16,
    KwChar32,
    KwClass,
    KwConst,
    KwConsteval,
    KwConstexpr,
    KwConstinit,
    KwContinue,
    KwDecltype,
    KwDefault,
    KwDelete,
    KwDo,
    KwDouble,
    KwDynamicCast,
    KwElse,
    KwEnum,
    KwExplicit,
    KwExport,
    KwExtern,
    KwFalse,
    KwFloat,
    KwFor,
    KwFriend,
    KwGoto,
    KwIf,
    KwInline,
    KwInt,
    KwLong,
    KwMutable,
    KwNamespace,
    KwNew,
    KwNoexcept,
    KwNullptr,
    KwOperator,
    KwPrivate,
    KwProtected,
    KwPublic,
    KwRegister,
    KwReinterpretCast,
    KwReturn,
    KwShort,
    KwSigned,
    KwSizeof,
    KwStatic,
    KwStaticAssert,
    KwStruct,
    KwSwitch,
    KwTemplate,
    KwThis,
    KwThreadLocal,
    KwThrow,
    KwTrue,
    KwTry,
    KwTypedef,
    KwTypeid,
    KwTypename,
    KwUnion,
    KwUnsigned,
    KwUsing,
    KwVirtual,
    KwVoid,
    KwVolatile,
    KwWchar,
    KwWhile,
    KwConcept,
    KwRequires,
    KwCoAwait,
    KwCoReturn,
    KwCoYield,
    KwModule,
    KwImport,

    // Metal Keywords / qualifiers
    KwKernel,
    KwVertex,
    KwFragment,
    KwMesh,
    KwObject,
    KwDevice,
    KwThreadgroup,
    KwConstant,
    KwThread,
    KwRayData,
    KwVisible,
    KwSampler,
    KwTexture,
    KwHalf,
    KwBFloat,
    KwBFloat16,

    // Composite Nodes (Parser output)
    Root,
    FunctionDef,
    ParameterList,
    Parameter,
    Block,
    StructDef,
    FieldDef,
    ClassDef,
    EnumDef,
    NamespaceDef,
    TemplateDef,
    TemplateParameter,
    TypedefDef,
    UsingDef,
    PreprocInclude,
    PreprocDefine,
    PreprocIf,
    PreprocIfdef,
    PreprocIfndef,
    PreprocElse,
    PreprocElif,
    PreprocEndif,
    PreprocPragma,
    ReturnStmt,
    IfStmt,
    ForStmt,
    WhileStmt,
    SwitchStmt,
    CaseStmt,
    BreakStmt,
    ContinueStmt,
    DeclStmt,
    ExprStmt,
    AssignExpr,
    BinaryExpr,
    UnaryExpr,
    PostfixExpr,
    MemberExpr,
    IndexExpr,
    CallExpr,
    CastExpr,
    LiteralExpr,
    VariableDef,
    TypeRef,
    Attribute,
    AttributeArgList,
}

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        rowan::SyntaxKind(kind as u16)
    }
}

#[derive(Logos, Debug, PartialEq, Clone, Copy)]
#[logos(error = ())] // Use unit type for error
pub enum TokenKind {
    #[regex(r"[ \t\n\f]+")]
    Whitespace,

    #[regex(r"//.*", allow_greedy = true)]
    #[regex(r"/\*([^*]|\*+[^*/])*\*+/")]
    Comment,

    // Preprocessor tokens
    #[token("##")]
    HashHash,
    #[token("#")]
    Hash,

    // Punctuation
    #[token("[[")]
    LDoubleBracket,
    #[token("]]")]
    RDoubleBracket,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(";")]
    Semicolon,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token("...")]
    Ellipsis,
    #[token("->*")]
    ArrowStar,
    #[token("->")]
    Arrow,
    #[token(".*")]
    DotStar,
    #[token(".")]
    Dot,
    #[token("::")]
    DoubleColon,
    #[token("?")]
    Question,

    // Operators (multi-char first)
    #[token(">>=")]
    RightShiftEqual,
    #[token("<<=")]
    LeftShiftEqual,
    #[token("++")]
    PlusPlus,
    #[token("--")]
    MinusMinus,
    #[token("+=")]
    PlusEqual,
    #[token("-=")]
    MinusEqual,
    #[token("*=")]
    StarEqual,
    #[token("/=")]
    SlashEqual,
    #[token("%=")]
    PercentEqual,
    #[token("&=")]
    AmpEqual,
    #[token("|=")]
    PipeEqual,
    #[token("^=")]
    CaretEqual,
    #[token("==")]
    EqualEqual,
    #[token("!=")]
    NotEqual,
    #[token("<=")]
    LessEqual,
    #[token(">=")]
    GreaterEqual,
    #[token("&&")]
    AndAnd,
    #[token("||")]
    OrOr,
    #[token("<<")]
    LeftShift,
    #[token(">>")]
    RightShift,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("^")]
    Caret,
    #[token("&")]
    Amp,
    #[token("|")]
    Pipe,
    #[token("~")]
    Tilde,
    #[token("!")]
    Exclaim,
    #[token("=")]
    Equal,
    #[token("<")]
    Less,
    #[token(">")]
    Greater,

    // Keywords (C/C++)
    #[token("alignas")]
    KwAlignas,
    #[token("alignof")]
    KwAlignof,
    #[token("asm")]
    KwAsm,
    #[token("auto")]
    KwAuto,
    #[token("bool")]
    KwBool,
    #[token("break")]
    KwBreak,
    #[token("case")]
    KwCase,
    #[token("catch")]
    KwCatch,
    #[token("char")]
    KwChar,
    #[token("char8_t")]
    KwChar8,
    #[token("char16_t")]
    KwChar16,
    #[token("char32_t")]
    KwChar32,
    #[token("class")]
    KwClass,
    #[token("const")]
    KwConst,
    #[token("consteval")]
    KwConsteval,
    #[token("constexpr")]
    KwConstexpr,
    #[token("constinit")]
    KwConstinit,
    #[token("continue")]
    KwContinue,
    #[token("decltype")]
    KwDecltype,
    #[token("default")]
    KwDefault,
    #[token("delete")]
    KwDelete,
    #[token("do")]
    KwDo,
    #[token("double")]
    KwDouble,
    #[token("dynamic_cast")]
    KwDynamicCast,
    #[token("else")]
    KwElse,
    #[token("enum")]
    KwEnum,
    #[token("explicit")]
    KwExplicit,
    #[token("export")]
    KwExport,
    #[token("extern")]
    KwExtern,
    #[token("false")]
    KwFalse,
    #[token("float")]
    KwFloat,
    #[token("for")]
    KwFor,
    #[token("friend")]
    KwFriend,
    #[token("goto")]
    KwGoto,
    #[token("if")]
    KwIf,
    #[token("inline")]
    KwInline,
    #[token("int")]
    KwInt,
    #[token("long")]
    KwLong,
    #[token("mutable")]
    KwMutable,
    #[token("namespace")]
    KwNamespace,
    #[token("new")]
    KwNew,
    #[token("noexcept")]
    KwNoexcept,
    #[token("nullptr")]
    KwNullptr,
    #[token("operator")]
    KwOperator,
    #[token("private")]
    KwPrivate,
    #[token("protected")]
    KwProtected,
    #[token("public")]
    KwPublic,
    #[token("register")]
    KwRegister,
    #[token("reinterpret_cast")]
    KwReinterpretCast,
    #[token("return")]
    KwReturn,
    #[token("short")]
    KwShort,
    #[token("signed")]
    KwSigned,
    #[token("sizeof")]
    KwSizeof,
    #[token("static")]
    KwStatic,
    #[token("static_assert")]
    KwStaticAssert,
    #[token("struct")]
    KwStruct,
    #[token("switch")]
    KwSwitch,
    #[token("template")]
    KwTemplate,
    #[token("this")]
    KwThis,
    #[token("thread_local")]
    KwThreadLocal,
    #[token("throw")]
    KwThrow,
    #[token("true")]
    KwTrue,
    #[token("try")]
    KwTry,
    #[token("typedef")]
    KwTypedef,
    #[token("typeid")]
    KwTypeid,
    #[token("typename")]
    KwTypename,
    #[token("union")]
    KwUnion,
    #[token("unsigned")]
    KwUnsigned,
    #[token("using")]
    KwUsing,
    #[token("virtual")]
    KwVirtual,
    #[token("void")]
    KwVoid,
    #[token("volatile")]
    KwVolatile,
    #[token("wchar_t")]
    KwWchar,
    #[token("while")]
    KwWhile,
    #[token("concept")]
    KwConcept,
    #[token("requires")]
    KwRequires,
    #[token("co_await")]
    KwCoAwait,
    #[token("co_return")]
    KwCoReturn,
    #[token("co_yield")]
    KwCoYield,
    #[token("module")]
    KwModule,
    #[token("import")]
    KwImport,

    // Metal keywords / qualifiers
    #[token("kernel")]
    KwKernel,
    #[token("vertex")]
    KwVertex,
    #[token("fragment")]
    KwFragment,
    #[token("mesh")]
    KwMesh,
    #[token("object")]
    KwObject,
    #[token("device")]
    KwDevice,
    #[token("threadgroup")]
    KwThreadgroup,
    #[token("constant")]
    KwConstant,
    #[token("thread")]
    KwThread,
    #[token("ray_data")]
    KwRayData,
    #[token("visible")]
    KwVisible,
    #[token("sampler")]
    KwSampler,
    #[token("texture")]
    KwTexture,
    #[token("half")]
    KwHalf,
    #[token("bfloat")]
    KwBFloat,
    #[token("bfloat16_t")]
    KwBFloat16,

    // Literals
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident,
    #[regex(r#"'([^'\\]|\\[\s\S])'"#)]
    Char,
    #[regex(r#""([^"\\]|\\[\s\S])*""#)]
    String,
    #[regex(r#"R"([^"]*)""#)]
    RawString,
    #[regex(r"0[xX][0-9A-Fa-f](_?[0-9A-Fa-f])*([uUlL]+)?")]
    #[regex(r"0[bB][01](_?[01])*([uUlL]+)?")]
    #[regex(r"0[0-7](_?[0-7])*([uUlL]+)?")]
    #[regex(r"[0-9](_?[0-9])*([uUlL]+)?")]
    Integer,
    #[regex(r"[0-9](_?[0-9])*\.[0-9](_?[0-9])*([eE][+-]?[0-9](_?[0-9])*)?([fFlL]+)?")]
    #[regex(r"\.[0-9](_?[0-9])*([eE][+-]?[0-9](_?[0-9])*)?([fFlL]+)?")]
    #[regex(r"[0-9](_?[0-9])*[eE][+-]?[0-9](_?[0-9])*([fFlL]+)?")]
    Float,
}

impl From<TokenKind> for SyntaxKind {
    fn from(token: TokenKind) -> Self {
        match token {
            TokenKind::Whitespace => SyntaxKind::Whitespace,
            TokenKind::Comment => SyntaxKind::Comment,
            TokenKind::Hash => SyntaxKind::Hash,
            TokenKind::HashHash => SyntaxKind::HashHash,
            TokenKind::LDoubleBracket => SyntaxKind::LDoubleBracket,
            TokenKind::RDoubleBracket => SyntaxKind::RDoubleBracket,
            TokenKind::LParen => SyntaxKind::LParen,
            TokenKind::RParen => SyntaxKind::RParen,
            TokenKind::LBrace => SyntaxKind::LBrace,
            TokenKind::RBrace => SyntaxKind::RBrace,
            TokenKind::LBracket => SyntaxKind::LBracket,
            TokenKind::RBracket => SyntaxKind::RBracket,
            TokenKind::Semicolon => SyntaxKind::Semicolon,
            TokenKind::Colon => SyntaxKind::Colon,
            TokenKind::Comma => SyntaxKind::Comma,
            TokenKind::Ellipsis => SyntaxKind::Ellipsis,
            TokenKind::ArrowStar => SyntaxKind::ArrowStar,
            TokenKind::Arrow => SyntaxKind::Arrow,
            TokenKind::DotStar => SyntaxKind::DotStar,
            TokenKind::Dot => SyntaxKind::Dot,
            TokenKind::DoubleColon => SyntaxKind::DoubleColon,
            TokenKind::Question => SyntaxKind::Question,
            TokenKind::RightShiftEqual => SyntaxKind::RightShiftEqual,
            TokenKind::LeftShiftEqual => SyntaxKind::LeftShiftEqual,
            TokenKind::PlusPlus => SyntaxKind::PlusPlus,
            TokenKind::MinusMinus => SyntaxKind::MinusMinus,
            TokenKind::PlusEqual => SyntaxKind::PlusEqual,
            TokenKind::MinusEqual => SyntaxKind::MinusEqual,
            TokenKind::StarEqual => SyntaxKind::StarEqual,
            TokenKind::SlashEqual => SyntaxKind::SlashEqual,
            TokenKind::PercentEqual => SyntaxKind::PercentEqual,
            TokenKind::AmpEqual => SyntaxKind::AmpEqual,
            TokenKind::PipeEqual => SyntaxKind::PipeEqual,
            TokenKind::CaretEqual => SyntaxKind::CaretEqual,
            TokenKind::EqualEqual => SyntaxKind::EqualEqual,
            TokenKind::NotEqual => SyntaxKind::NotEqual,
            TokenKind::LessEqual => SyntaxKind::LessEqual,
            TokenKind::GreaterEqual => SyntaxKind::GreaterEqual,
            TokenKind::AndAnd => SyntaxKind::AndAnd,
            TokenKind::OrOr => SyntaxKind::OrOr,
            TokenKind::LeftShift => SyntaxKind::LeftShift,
            TokenKind::RightShift => SyntaxKind::RightShift,
            TokenKind::Plus => SyntaxKind::Plus,
            TokenKind::Minus => SyntaxKind::Minus,
            TokenKind::Star => SyntaxKind::Star,
            TokenKind::Slash => SyntaxKind::Slash,
            TokenKind::Percent => SyntaxKind::Percent,
            TokenKind::Caret => SyntaxKind::Caret,
            TokenKind::Amp => SyntaxKind::Amp,
            TokenKind::Pipe => SyntaxKind::Pipe,
            TokenKind::Tilde => SyntaxKind::Tilde,
            TokenKind::Exclaim => SyntaxKind::Exclaim,
            TokenKind::Equal => SyntaxKind::Equal,
            TokenKind::Less => SyntaxKind::Less,
            TokenKind::Greater => SyntaxKind::Greater,
            TokenKind::KwAlignas => SyntaxKind::KwAlignas,
            TokenKind::KwAlignof => SyntaxKind::KwAlignof,
            TokenKind::KwAsm => SyntaxKind::KwAsm,
            TokenKind::KwAuto => SyntaxKind::KwAuto,
            TokenKind::KwBool => SyntaxKind::KwBool,
            TokenKind::KwBreak => SyntaxKind::KwBreak,
            TokenKind::KwCase => SyntaxKind::KwCase,
            TokenKind::KwCatch => SyntaxKind::KwCatch,
            TokenKind::KwChar => SyntaxKind::KwChar,
            TokenKind::KwChar8 => SyntaxKind::KwChar8,
            TokenKind::KwChar16 => SyntaxKind::KwChar16,
            TokenKind::KwChar32 => SyntaxKind::KwChar32,
            TokenKind::KwClass => SyntaxKind::KwClass,
            TokenKind::KwConst => SyntaxKind::KwConst,
            TokenKind::KwConsteval => SyntaxKind::KwConsteval,
            TokenKind::KwConstexpr => SyntaxKind::KwConstexpr,
            TokenKind::KwConstinit => SyntaxKind::KwConstinit,
            TokenKind::KwContinue => SyntaxKind::KwContinue,
            TokenKind::KwDecltype => SyntaxKind::KwDecltype,
            TokenKind::KwDefault => SyntaxKind::KwDefault,
            TokenKind::KwDelete => SyntaxKind::KwDelete,
            TokenKind::KwDo => SyntaxKind::KwDo,
            TokenKind::KwDouble => SyntaxKind::KwDouble,
            TokenKind::KwDynamicCast => SyntaxKind::KwDynamicCast,
            TokenKind::KwElse => SyntaxKind::KwElse,
            TokenKind::KwEnum => SyntaxKind::KwEnum,
            TokenKind::KwExplicit => SyntaxKind::KwExplicit,
            TokenKind::KwExport => SyntaxKind::KwExport,
            TokenKind::KwExtern => SyntaxKind::KwExtern,
            TokenKind::KwFalse => SyntaxKind::KwFalse,
            TokenKind::KwFloat => SyntaxKind::KwFloat,
            TokenKind::KwFor => SyntaxKind::KwFor,
            TokenKind::KwFriend => SyntaxKind::KwFriend,
            TokenKind::KwGoto => SyntaxKind::KwGoto,
            TokenKind::KwIf => SyntaxKind::KwIf,
            TokenKind::KwInline => SyntaxKind::KwInline,
            TokenKind::KwInt => SyntaxKind::KwInt,
            TokenKind::KwLong => SyntaxKind::KwLong,
            TokenKind::KwMutable => SyntaxKind::KwMutable,
            TokenKind::KwNamespace => SyntaxKind::KwNamespace,
            TokenKind::KwNew => SyntaxKind::KwNew,
            TokenKind::KwNoexcept => SyntaxKind::KwNoexcept,
            TokenKind::KwNullptr => SyntaxKind::KwNullptr,
            TokenKind::KwOperator => SyntaxKind::KwOperator,
            TokenKind::KwPrivate => SyntaxKind::KwPrivate,
            TokenKind::KwProtected => SyntaxKind::KwProtected,
            TokenKind::KwPublic => SyntaxKind::KwPublic,
            TokenKind::KwRegister => SyntaxKind::KwRegister,
            TokenKind::KwReinterpretCast => SyntaxKind::KwReinterpretCast,
            TokenKind::KwReturn => SyntaxKind::KwReturn,
            TokenKind::KwShort => SyntaxKind::KwShort,
            TokenKind::KwSigned => SyntaxKind::KwSigned,
            TokenKind::KwSizeof => SyntaxKind::KwSizeof,
            TokenKind::KwStatic => SyntaxKind::KwStatic,
            TokenKind::KwStaticAssert => SyntaxKind::KwStaticAssert,
            TokenKind::KwStruct => SyntaxKind::KwStruct,
            TokenKind::KwSwitch => SyntaxKind::KwSwitch,
            TokenKind::KwTemplate => SyntaxKind::KwTemplate,
            TokenKind::KwThis => SyntaxKind::KwThis,
            TokenKind::KwThreadLocal => SyntaxKind::KwThreadLocal,
            TokenKind::KwThrow => SyntaxKind::KwThrow,
            TokenKind::KwTrue => SyntaxKind::KwTrue,
            TokenKind::KwTry => SyntaxKind::KwTry,
            TokenKind::KwTypedef => SyntaxKind::KwTypedef,
            TokenKind::KwTypeid => SyntaxKind::KwTypeid,
            TokenKind::KwTypename => SyntaxKind::KwTypename,
            TokenKind::KwUnion => SyntaxKind::KwUnion,
            TokenKind::KwUnsigned => SyntaxKind::KwUnsigned,
            TokenKind::KwUsing => SyntaxKind::KwUsing,
            TokenKind::KwVirtual => SyntaxKind::KwVirtual,
            TokenKind::KwVoid => SyntaxKind::KwVoid,
            TokenKind::KwVolatile => SyntaxKind::KwVolatile,
            TokenKind::KwWchar => SyntaxKind::KwWchar,
            TokenKind::KwWhile => SyntaxKind::KwWhile,
            TokenKind::KwConcept => SyntaxKind::KwConcept,
            TokenKind::KwRequires => SyntaxKind::KwRequires,
            TokenKind::KwCoAwait => SyntaxKind::KwCoAwait,
            TokenKind::KwCoReturn => SyntaxKind::KwCoReturn,
            TokenKind::KwCoYield => SyntaxKind::KwCoYield,
            TokenKind::KwModule => SyntaxKind::KwModule,
            TokenKind::KwImport => SyntaxKind::KwImport,
            TokenKind::KwKernel => SyntaxKind::KwKernel,
            TokenKind::KwVertex => SyntaxKind::KwVertex,
            TokenKind::KwFragment => SyntaxKind::KwFragment,
            TokenKind::KwMesh => SyntaxKind::KwMesh,
            TokenKind::KwObject => SyntaxKind::KwObject,
            TokenKind::KwDevice => SyntaxKind::KwDevice,
            TokenKind::KwThreadgroup => SyntaxKind::KwThreadgroup,
            TokenKind::KwConstant => SyntaxKind::KwConstant,
            TokenKind::KwThread => SyntaxKind::KwThread,
            TokenKind::KwRayData => SyntaxKind::KwRayData,
            TokenKind::KwVisible => SyntaxKind::KwVisible,
            TokenKind::KwSampler => SyntaxKind::KwSampler,
            TokenKind::KwTexture => SyntaxKind::KwTexture,
            TokenKind::KwHalf => SyntaxKind::KwHalf,
            TokenKind::KwBFloat => SyntaxKind::KwBFloat,
            TokenKind::KwBFloat16 => SyntaxKind::KwBFloat16,
            TokenKind::Ident => SyntaxKind::Ident,
            TokenKind::Char => SyntaxKind::Char,
            TokenKind::String => SyntaxKind::String,
            TokenKind::RawString => SyntaxKind::RawString,
            TokenKind::Integer => SyntaxKind::Integer,
            TokenKind::Float => SyntaxKind::Float,
        }
    }
}
