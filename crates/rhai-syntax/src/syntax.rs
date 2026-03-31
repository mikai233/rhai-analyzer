pub use text_size::{TextRange, TextSize};

use std::sync::Arc;

use rowan::Language;
use thiserror::Error;

use crate::TriviaStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    Whitespace,          // spaces, tabs, newlines
    LineComment,         // `// ...`
    DocLineComment,      // `/// ...` or `//! ...`
    BlockComment,        // `/* ... */`
    DocBlockComment,     // `/** ... */` or `/*! ... */`
    Shebang,             // `#!...` at the start of the file
    Ident,               // identifier
    Underscore,          // `_`
    Int,                 // integer literal
    Float,               // float literal
    String,              // `"..."` string literal
    RawString,           // raw string literal
    BacktickString,      // `` `...` `` string literal
    Backtick,            // `` ` `` inside interpolated string structure
    StringText,          // plain text segment inside interpolated string
    InterpolationStart,  // `${`
    Char,                // `'x'` character literal
    LetKw,               // `let`
    ConstKw,             // `const`
    IfKw,                // `if`
    ElseKw,              // `else`
    SwitchKw,            // `switch`
    DoKw,                // `do`
    WhileKw,             // `while`
    UntilKw,             // `until`
    LoopKw,              // `loop`
    ForKw,               // `for`
    InKw,                // `in`
    ContinueKw,          // `continue`
    BreakKw,             // `break`
    ReturnKw,            // `return`
    ThrowKw,             // `throw`
    TryKw,               // `try`
    CatchKw,             // `catch`
    ImportKw,            // `import`
    ExportKw,            // `export`
    AsKw,                // `as`
    GlobalKw,            // `global`
    PrivateKw,           // `private`
    FnKw,                // `fn`
    ThisKw,              // `this`
    TrueKw,              // `true`
    FalseKw,             // `false`
    FnPtrKw,             // `Fn`
    CallKw,              // `call`
    CurryKw,             // `curry`
    IsSharedKw,          // `is_shared`
    IsDefFnKw,           // `is_def_fn`
    IsDefVarKw,          // `is_def_var`
    TypeOfKw,            // `type_of`
    PrintKw,             // `print`
    DebugKw,             // `debug`
    EvalKw,              // `eval`
    OpenParen,           // `(`
    CloseParen,          // `)`
    OpenBrace,           // `{`
    CloseBrace,          // `}`
    OpenBracket,         // `[`
    CloseBracket,        // `]`
    HashBraceOpen,       // `#{`
    Comma,               // `,`
    Semicolon,           // `;`
    Colon,               // `:`
    ColonColon,          // `::`
    Dot,                 // `.`
    QuestionDot,         // `?.`
    QuestionOpenBracket, // `?[`
    FatArrow,            // `=>`
    Eq,                  // `=`
    PlusEq,              // `+=`
    MinusEq,             // `-=`
    StarEq,              // `*=`
    SlashEq,             // `/=`
    PercentEq,           // `%=`
    StarStarEq,          // `**=`
    ShlEq,               // `<<=`
    ShrEq,               // `>>=`
    AmpEq,               // `&=`
    PipeEq,              // `|=`
    CaretEq,             // `^=`
    QuestionQuestionEq,  // `??=`
    Plus,                // `+`
    Minus,               // `-`
    Star,                // `*`
    Slash,               // `/`
    Percent,             // `%`
    StarStar,            // `**`
    Shl,                 // `<<`
    Shr,                 // `>>`
    Amp,                 // `&`
    Pipe,                // `|`
    Caret,               // `^`
    AmpAmp,              // `&&`
    PipePipe,            // `||`
    Bang,                // `!`
    EqEq,                // `==`
    BangEq,              // `!=`
    Gt,                  // `>`
    GtEq,                // `>=`
    Lt,                  // `<`
    LtEq,                // `<=`
    QuestionQuestion,    // `??`
    Range,               // `..`
    RangeEq,             // `..=`
    Unknown,             // unrecognized token
}

impl TokenKind {
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::Whitespace
                | Self::LineComment
                | Self::DocLineComment
                | Self::BlockComment
                | Self::DocBlockComment
                | Self::Shebang
        )
    }

    pub const fn to_rowan_kind(self) -> RhaiKind {
        RhaiKind::from_token(self)
    }

    pub const fn to_rowan(self) -> RowanSyntaxKind {
        self.to_rowan_kind().to_rowan()
    }

    pub fn from_rowan(raw: RowanSyntaxKind) -> Option<Self> {
        RhaiKind::from_rowan(raw).and_then(RhaiKind::token_kind)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SyntaxToken {
    kind: TokenKind,
    range: TextRange,
}

impl SyntaxToken {
    pub fn new(kind: TokenKind, range: TextRange) -> Self {
        Self { kind, range }
    }

    pub fn kind(self) -> TokenKind {
        self.kind
    }

    pub fn range(self) -> TextRange {
        self.range
    }

    pub fn text(self, source: &str) -> &str {
        let start = u32::from(self.range.start()) as usize;
        let end = u32::from(self.range.end()) as usize;
        &source[start..end]
    }
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SyntaxKind {
    Root,                   // whole file / parse root
    ItemFn,                 // function item
    StmtLet,                // `let` statement
    StmtConst,              // `const` statement
    StmtImport,             // `import` statement
    StmtExport,             // `export` statement
    StmtBreak,              // `break` statement
    StmtContinue,           // `continue` statement
    StmtReturn,             // `return` statement
    StmtThrow,              // `throw` statement
    StmtTry,                // `try` statement
    StmtExpr,               // expression statement
    ExprName,               // name / identifier expression
    ExprLiteral,            // literal expression
    ExprArray,              // array literal
    ExprObject,             // object map literal
    ExprIf,                 // `if` expression
    ExprSwitch,             // `switch` expression
    ExprWhile,              // `while` expression
    ExprLoop,               // `loop` expression
    ExprFor,                // `for` expression
    ExprDo,                 // `do ... while/until` expression
    ExprPath,               // `a::b::c` path expression
    ExprClosure,            // closure expression
    ExprInterpolatedString, // interpolated back-tick string
    ExprUnary,              // unary operator expression
    ExprBinary,             // binary operator expression
    ExprAssign,             // assignment / compound assignment expression
    ExprParen,              // parenthesized expression
    ExprCall,               // function or method call expression
    ExprIndex,              // indexing / safe indexing expression
    ExprField,              // field / property access expression
    RootItemList,           // root-level item list
    BlockItemList,          // block item list
    ArgList,                // call argument list
    ParamList,              // function parameter list
    ClosureParamList,       // closure parameter list
    ArrayItemList,          // array element list
    ObjectFieldList,        // object field list
    ObjectField,            // object field entry
    SwitchArmList,          // switch arm list
    StringPartList,         // parts inside an interpolated string
    StringSegment,          // plain text segment inside interpolation
    StringInterpolation,    // `${ ... }` interpolation segment
    InterpolationBody,      // parsed body inside `${ ... }`
    InterpolationItemList,  // item list inside `${ ... }`
    ElseBranch,             // `else` branch
    ForBindings,            // binding list in `for`
    AliasClause,            // `as alias`
    SwitchArm,              // one `switch` arm
    SwitchPatternList,      // pattern list before `=>`
    DoCondition,            // trailing `while/until` condition
    CatchClause,            // `catch` clause
    Block,                  // `{ ... }` block
    Error,                  // recovery / missing node
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RhaiKind {
    Whitespace,
    LineComment,
    DocLineComment,
    BlockComment,
    DocBlockComment,
    Shebang,
    Ident,
    Underscore,
    Int,
    Float,
    String,
    RawString,
    BacktickString,
    Backtick,
    StringText,
    InterpolationStart,
    Char,
    LetKw,
    ConstKw,
    IfKw,
    ElseKw,
    SwitchKw,
    DoKw,
    WhileKw,
    UntilKw,
    LoopKw,
    ForKw,
    InKw,
    ContinueKw,
    BreakKw,
    ReturnKw,
    ThrowKw,
    TryKw,
    CatchKw,
    ImportKw,
    ExportKw,
    AsKw,
    GlobalKw,
    PrivateKw,
    FnKw,
    ThisKw,
    TrueKw,
    FalseKw,
    FnPtrKw,
    CallKw,
    CurryKw,
    IsSharedKw,
    IsDefFnKw,
    IsDefVarKw,
    TypeOfKw,
    PrintKw,
    DebugKw,
    EvalKw,
    OpenParen,
    CloseParen,
    OpenBrace,
    CloseBrace,
    OpenBracket,
    CloseBracket,
    HashBraceOpen,
    Comma,
    Semicolon,
    Colon,
    ColonColon,
    Dot,
    QuestionDot,
    QuestionOpenBracket,
    FatArrow,
    Eq,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
    StarStarEq,
    ShlEq,
    ShrEq,
    AmpEq,
    PipeEq,
    CaretEq,
    QuestionQuestionEq,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    StarStar,
    Shl,
    Shr,
    Amp,
    Pipe,
    Caret,
    AmpAmp,
    PipePipe,
    Bang,
    EqEq,
    BangEq,
    Gt,
    GtEq,
    Lt,
    LtEq,
    QuestionQuestion,
    Range,
    RangeEq,
    Unknown,
    Root,
    ItemFn,
    StmtLet,
    StmtConst,
    StmtImport,
    StmtExport,
    StmtBreak,
    StmtContinue,
    StmtReturn,
    StmtThrow,
    StmtTry,
    StmtExpr,
    ExprName,
    ExprLiteral,
    ExprArray,
    ExprObject,
    ExprIf,
    ExprSwitch,
    ExprWhile,
    ExprLoop,
    ExprFor,
    ExprDo,
    ExprPath,
    ExprClosure,
    ExprInterpolatedString,
    ExprUnary,
    ExprBinary,
    ExprAssign,
    ExprParen,
    ExprCall,
    ExprIndex,
    ExprField,
    RootItemList,
    BlockItemList,
    ArgList,
    ParamList,
    ClosureParamList,
    ArrayItemList,
    ObjectFieldList,
    ObjectField,
    SwitchArmList,
    StringPartList,
    StringSegment,
    StringInterpolation,
    InterpolationBody,
    InterpolationItemList,
    ElseBranch,
    ForBindings,
    AliasClause,
    SwitchArm,
    SwitchPatternList,
    DoCondition,
    CatchClause,
    Block,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RhaiLanguage {}

pub type RowanSyntaxKind = rowan::SyntaxKind;
pub type RowanSyntaxNode = rowan::SyntaxNode<RhaiLanguage>;
pub type RowanSyntaxElement = rowan::SyntaxElement<RhaiLanguage>;
pub type RowanSyntaxToken = rowan::SyntaxToken<RhaiLanguage>;
pub type RowanNodeOrToken = rowan::NodeOrToken<RowanSyntaxNode, RowanSyntaxToken>;
pub type RowanGreenNode = rowan::GreenNode;
pub type RowanGreenToken = rowan::GreenToken;

impl SyntaxKind {
    pub const fn to_rowan_kind(self) -> RhaiKind {
        RhaiKind::from_syntax(self)
    }

    pub const fn to_rowan(self) -> RowanSyntaxKind {
        self.to_rowan_kind().to_rowan()
    }

    pub fn from_rowan(kind: RowanSyntaxKind) -> Option<Self> {
        RhaiKind::from_rowan(kind).and_then(RhaiKind::syntax_kind)
    }
}

impl RhaiKind {
    pub const fn to_rowan(self) -> RowanSyntaxKind {
        rowan::SyntaxKind(self as u16)
    }

    pub fn from_rowan(kind: RowanSyntaxKind) -> Option<Self> {
        if kind.0 <= Self::Error as u16 {
            // SAFETY: `RhaiKind` uses a dense `repr(u16)` layout and the bound
            // check above ensures the raw discriminant maps to a declared variant.
            Some(unsafe { std::mem::transmute::<u16, Self>(kind.0) })
        } else {
            None
        }
    }

    pub const fn from_token(kind: TokenKind) -> Self {
        match kind {
            TokenKind::Whitespace => Self::Whitespace,
            TokenKind::LineComment => Self::LineComment,
            TokenKind::DocLineComment => Self::DocLineComment,
            TokenKind::BlockComment => Self::BlockComment,
            TokenKind::DocBlockComment => Self::DocBlockComment,
            TokenKind::Shebang => Self::Shebang,
            TokenKind::Ident => Self::Ident,
            TokenKind::Underscore => Self::Underscore,
            TokenKind::Int => Self::Int,
            TokenKind::Float => Self::Float,
            TokenKind::String => Self::String,
            TokenKind::RawString => Self::RawString,
            TokenKind::BacktickString => Self::BacktickString,
            TokenKind::Backtick => Self::Backtick,
            TokenKind::StringText => Self::StringText,
            TokenKind::InterpolationStart => Self::InterpolationStart,
            TokenKind::Char => Self::Char,
            TokenKind::LetKw => Self::LetKw,
            TokenKind::ConstKw => Self::ConstKw,
            TokenKind::IfKw => Self::IfKw,
            TokenKind::ElseKw => Self::ElseKw,
            TokenKind::SwitchKw => Self::SwitchKw,
            TokenKind::DoKw => Self::DoKw,
            TokenKind::WhileKw => Self::WhileKw,
            TokenKind::UntilKw => Self::UntilKw,
            TokenKind::LoopKw => Self::LoopKw,
            TokenKind::ForKw => Self::ForKw,
            TokenKind::InKw => Self::InKw,
            TokenKind::ContinueKw => Self::ContinueKw,
            TokenKind::BreakKw => Self::BreakKw,
            TokenKind::ReturnKw => Self::ReturnKw,
            TokenKind::ThrowKw => Self::ThrowKw,
            TokenKind::TryKw => Self::TryKw,
            TokenKind::CatchKw => Self::CatchKw,
            TokenKind::ImportKw => Self::ImportKw,
            TokenKind::ExportKw => Self::ExportKw,
            TokenKind::AsKw => Self::AsKw,
            TokenKind::GlobalKw => Self::GlobalKw,
            TokenKind::PrivateKw => Self::PrivateKw,
            TokenKind::FnKw => Self::FnKw,
            TokenKind::ThisKw => Self::ThisKw,
            TokenKind::TrueKw => Self::TrueKw,
            TokenKind::FalseKw => Self::FalseKw,
            TokenKind::FnPtrKw => Self::FnPtrKw,
            TokenKind::CallKw => Self::CallKw,
            TokenKind::CurryKw => Self::CurryKw,
            TokenKind::IsSharedKw => Self::IsSharedKw,
            TokenKind::IsDefFnKw => Self::IsDefFnKw,
            TokenKind::IsDefVarKw => Self::IsDefVarKw,
            TokenKind::TypeOfKw => Self::TypeOfKw,
            TokenKind::PrintKw => Self::PrintKw,
            TokenKind::DebugKw => Self::DebugKw,
            TokenKind::EvalKw => Self::EvalKw,
            TokenKind::OpenParen => Self::OpenParen,
            TokenKind::CloseParen => Self::CloseParen,
            TokenKind::OpenBrace => Self::OpenBrace,
            TokenKind::CloseBrace => Self::CloseBrace,
            TokenKind::OpenBracket => Self::OpenBracket,
            TokenKind::CloseBracket => Self::CloseBracket,
            TokenKind::HashBraceOpen => Self::HashBraceOpen,
            TokenKind::Comma => Self::Comma,
            TokenKind::Semicolon => Self::Semicolon,
            TokenKind::Colon => Self::Colon,
            TokenKind::ColonColon => Self::ColonColon,
            TokenKind::Dot => Self::Dot,
            TokenKind::QuestionDot => Self::QuestionDot,
            TokenKind::QuestionOpenBracket => Self::QuestionOpenBracket,
            TokenKind::FatArrow => Self::FatArrow,
            TokenKind::Eq => Self::Eq,
            TokenKind::PlusEq => Self::PlusEq,
            TokenKind::MinusEq => Self::MinusEq,
            TokenKind::StarEq => Self::StarEq,
            TokenKind::SlashEq => Self::SlashEq,
            TokenKind::PercentEq => Self::PercentEq,
            TokenKind::StarStarEq => Self::StarStarEq,
            TokenKind::ShlEq => Self::ShlEq,
            TokenKind::ShrEq => Self::ShrEq,
            TokenKind::AmpEq => Self::AmpEq,
            TokenKind::PipeEq => Self::PipeEq,
            TokenKind::CaretEq => Self::CaretEq,
            TokenKind::QuestionQuestionEq => Self::QuestionQuestionEq,
            TokenKind::Plus => Self::Plus,
            TokenKind::Minus => Self::Minus,
            TokenKind::Star => Self::Star,
            TokenKind::Slash => Self::Slash,
            TokenKind::Percent => Self::Percent,
            TokenKind::StarStar => Self::StarStar,
            TokenKind::Shl => Self::Shl,
            TokenKind::Shr => Self::Shr,
            TokenKind::Amp => Self::Amp,
            TokenKind::Pipe => Self::Pipe,
            TokenKind::Caret => Self::Caret,
            TokenKind::AmpAmp => Self::AmpAmp,
            TokenKind::PipePipe => Self::PipePipe,
            TokenKind::Bang => Self::Bang,
            TokenKind::EqEq => Self::EqEq,
            TokenKind::BangEq => Self::BangEq,
            TokenKind::Gt => Self::Gt,
            TokenKind::GtEq => Self::GtEq,
            TokenKind::Lt => Self::Lt,
            TokenKind::LtEq => Self::LtEq,
            TokenKind::QuestionQuestion => Self::QuestionQuestion,
            TokenKind::Range => Self::Range,
            TokenKind::RangeEq => Self::RangeEq,
            TokenKind::Unknown => Self::Unknown,
        }
    }

    pub const fn from_syntax(kind: SyntaxKind) -> Self {
        match kind {
            SyntaxKind::Root => Self::Root,
            SyntaxKind::ItemFn => Self::ItemFn,
            SyntaxKind::StmtLet => Self::StmtLet,
            SyntaxKind::StmtConst => Self::StmtConst,
            SyntaxKind::StmtImport => Self::StmtImport,
            SyntaxKind::StmtExport => Self::StmtExport,
            SyntaxKind::StmtBreak => Self::StmtBreak,
            SyntaxKind::StmtContinue => Self::StmtContinue,
            SyntaxKind::StmtReturn => Self::StmtReturn,
            SyntaxKind::StmtThrow => Self::StmtThrow,
            SyntaxKind::StmtTry => Self::StmtTry,
            SyntaxKind::StmtExpr => Self::StmtExpr,
            SyntaxKind::ExprName => Self::ExprName,
            SyntaxKind::ExprLiteral => Self::ExprLiteral,
            SyntaxKind::ExprArray => Self::ExprArray,
            SyntaxKind::ExprObject => Self::ExprObject,
            SyntaxKind::ExprIf => Self::ExprIf,
            SyntaxKind::ExprSwitch => Self::ExprSwitch,
            SyntaxKind::ExprWhile => Self::ExprWhile,
            SyntaxKind::ExprLoop => Self::ExprLoop,
            SyntaxKind::ExprFor => Self::ExprFor,
            SyntaxKind::ExprDo => Self::ExprDo,
            SyntaxKind::ExprPath => Self::ExprPath,
            SyntaxKind::ExprClosure => Self::ExprClosure,
            SyntaxKind::ExprInterpolatedString => Self::ExprInterpolatedString,
            SyntaxKind::ExprUnary => Self::ExprUnary,
            SyntaxKind::ExprBinary => Self::ExprBinary,
            SyntaxKind::ExprAssign => Self::ExprAssign,
            SyntaxKind::ExprParen => Self::ExprParen,
            SyntaxKind::ExprCall => Self::ExprCall,
            SyntaxKind::ExprIndex => Self::ExprIndex,
            SyntaxKind::ExprField => Self::ExprField,
            SyntaxKind::RootItemList => Self::RootItemList,
            SyntaxKind::BlockItemList => Self::BlockItemList,
            SyntaxKind::ArgList => Self::ArgList,
            SyntaxKind::ParamList => Self::ParamList,
            SyntaxKind::ClosureParamList => Self::ClosureParamList,
            SyntaxKind::ArrayItemList => Self::ArrayItemList,
            SyntaxKind::ObjectFieldList => Self::ObjectFieldList,
            SyntaxKind::ObjectField => Self::ObjectField,
            SyntaxKind::SwitchArmList => Self::SwitchArmList,
            SyntaxKind::StringPartList => Self::StringPartList,
            SyntaxKind::StringSegment => Self::StringSegment,
            SyntaxKind::StringInterpolation => Self::StringInterpolation,
            SyntaxKind::InterpolationBody => Self::InterpolationBody,
            SyntaxKind::InterpolationItemList => Self::InterpolationItemList,
            SyntaxKind::ElseBranch => Self::ElseBranch,
            SyntaxKind::ForBindings => Self::ForBindings,
            SyntaxKind::AliasClause => Self::AliasClause,
            SyntaxKind::SwitchArm => Self::SwitchArm,
            SyntaxKind::SwitchPatternList => Self::SwitchPatternList,
            SyntaxKind::DoCondition => Self::DoCondition,
            SyntaxKind::CatchClause => Self::CatchClause,
            SyntaxKind::Block => Self::Block,
            SyntaxKind::Error => Self::Error,
        }
    }

    pub const fn syntax_kind(self) -> Option<SyntaxKind> {
        match self {
            Self::Root => Some(SyntaxKind::Root),
            Self::ItemFn => Some(SyntaxKind::ItemFn),
            Self::StmtLet => Some(SyntaxKind::StmtLet),
            Self::StmtConst => Some(SyntaxKind::StmtConst),
            Self::StmtImport => Some(SyntaxKind::StmtImport),
            Self::StmtExport => Some(SyntaxKind::StmtExport),
            Self::StmtBreak => Some(SyntaxKind::StmtBreak),
            Self::StmtContinue => Some(SyntaxKind::StmtContinue),
            Self::StmtReturn => Some(SyntaxKind::StmtReturn),
            Self::StmtThrow => Some(SyntaxKind::StmtThrow),
            Self::StmtTry => Some(SyntaxKind::StmtTry),
            Self::StmtExpr => Some(SyntaxKind::StmtExpr),
            Self::ExprName => Some(SyntaxKind::ExprName),
            Self::ExprLiteral => Some(SyntaxKind::ExprLiteral),
            Self::ExprArray => Some(SyntaxKind::ExprArray),
            Self::ExprObject => Some(SyntaxKind::ExprObject),
            Self::ExprIf => Some(SyntaxKind::ExprIf),
            Self::ExprSwitch => Some(SyntaxKind::ExprSwitch),
            Self::ExprWhile => Some(SyntaxKind::ExprWhile),
            Self::ExprLoop => Some(SyntaxKind::ExprLoop),
            Self::ExprFor => Some(SyntaxKind::ExprFor),
            Self::ExprDo => Some(SyntaxKind::ExprDo),
            Self::ExprPath => Some(SyntaxKind::ExprPath),
            Self::ExprClosure => Some(SyntaxKind::ExprClosure),
            Self::ExprInterpolatedString => Some(SyntaxKind::ExprInterpolatedString),
            Self::ExprUnary => Some(SyntaxKind::ExprUnary),
            Self::ExprBinary => Some(SyntaxKind::ExprBinary),
            Self::ExprAssign => Some(SyntaxKind::ExprAssign),
            Self::ExprParen => Some(SyntaxKind::ExprParen),
            Self::ExprCall => Some(SyntaxKind::ExprCall),
            Self::ExprIndex => Some(SyntaxKind::ExprIndex),
            Self::ExprField => Some(SyntaxKind::ExprField),
            Self::RootItemList => Some(SyntaxKind::RootItemList),
            Self::BlockItemList => Some(SyntaxKind::BlockItemList),
            Self::ArgList => Some(SyntaxKind::ArgList),
            Self::ParamList => Some(SyntaxKind::ParamList),
            Self::ClosureParamList => Some(SyntaxKind::ClosureParamList),
            Self::ArrayItemList => Some(SyntaxKind::ArrayItemList),
            Self::ObjectFieldList => Some(SyntaxKind::ObjectFieldList),
            Self::ObjectField => Some(SyntaxKind::ObjectField),
            Self::SwitchArmList => Some(SyntaxKind::SwitchArmList),
            Self::StringPartList => Some(SyntaxKind::StringPartList),
            Self::StringSegment => Some(SyntaxKind::StringSegment),
            Self::StringInterpolation => Some(SyntaxKind::StringInterpolation),
            Self::InterpolationBody => Some(SyntaxKind::InterpolationBody),
            Self::InterpolationItemList => Some(SyntaxKind::InterpolationItemList),
            Self::ElseBranch => Some(SyntaxKind::ElseBranch),
            Self::ForBindings => Some(SyntaxKind::ForBindings),
            Self::AliasClause => Some(SyntaxKind::AliasClause),
            Self::SwitchArm => Some(SyntaxKind::SwitchArm),
            Self::SwitchPatternList => Some(SyntaxKind::SwitchPatternList),
            Self::DoCondition => Some(SyntaxKind::DoCondition),
            Self::CatchClause => Some(SyntaxKind::CatchClause),
            Self::Block => Some(SyntaxKind::Block),
            Self::Error => Some(SyntaxKind::Error),
            _ => None,
        }
    }

    pub const fn token_kind(self) -> Option<TokenKind> {
        match self {
            Self::Whitespace => Some(TokenKind::Whitespace),
            Self::LineComment => Some(TokenKind::LineComment),
            Self::DocLineComment => Some(TokenKind::DocLineComment),
            Self::BlockComment => Some(TokenKind::BlockComment),
            Self::DocBlockComment => Some(TokenKind::DocBlockComment),
            Self::Shebang => Some(TokenKind::Shebang),
            Self::Ident => Some(TokenKind::Ident),
            Self::Underscore => Some(TokenKind::Underscore),
            Self::Int => Some(TokenKind::Int),
            Self::Float => Some(TokenKind::Float),
            Self::String => Some(TokenKind::String),
            Self::RawString => Some(TokenKind::RawString),
            Self::BacktickString => Some(TokenKind::BacktickString),
            Self::Backtick => Some(TokenKind::Backtick),
            Self::StringText => Some(TokenKind::StringText),
            Self::InterpolationStart => Some(TokenKind::InterpolationStart),
            Self::Char => Some(TokenKind::Char),
            Self::LetKw => Some(TokenKind::LetKw),
            Self::ConstKw => Some(TokenKind::ConstKw),
            Self::IfKw => Some(TokenKind::IfKw),
            Self::ElseKw => Some(TokenKind::ElseKw),
            Self::SwitchKw => Some(TokenKind::SwitchKw),
            Self::DoKw => Some(TokenKind::DoKw),
            Self::WhileKw => Some(TokenKind::WhileKw),
            Self::UntilKw => Some(TokenKind::UntilKw),
            Self::LoopKw => Some(TokenKind::LoopKw),
            Self::ForKw => Some(TokenKind::ForKw),
            Self::InKw => Some(TokenKind::InKw),
            Self::ContinueKw => Some(TokenKind::ContinueKw),
            Self::BreakKw => Some(TokenKind::BreakKw),
            Self::ReturnKw => Some(TokenKind::ReturnKw),
            Self::ThrowKw => Some(TokenKind::ThrowKw),
            Self::TryKw => Some(TokenKind::TryKw),
            Self::CatchKw => Some(TokenKind::CatchKw),
            Self::ImportKw => Some(TokenKind::ImportKw),
            Self::ExportKw => Some(TokenKind::ExportKw),
            Self::AsKw => Some(TokenKind::AsKw),
            Self::GlobalKw => Some(TokenKind::GlobalKw),
            Self::PrivateKw => Some(TokenKind::PrivateKw),
            Self::FnKw => Some(TokenKind::FnKw),
            Self::ThisKw => Some(TokenKind::ThisKw),
            Self::TrueKw => Some(TokenKind::TrueKw),
            Self::FalseKw => Some(TokenKind::FalseKw),
            Self::FnPtrKw => Some(TokenKind::FnPtrKw),
            Self::CallKw => Some(TokenKind::CallKw),
            Self::CurryKw => Some(TokenKind::CurryKw),
            Self::IsSharedKw => Some(TokenKind::IsSharedKw),
            Self::IsDefFnKw => Some(TokenKind::IsDefFnKw),
            Self::IsDefVarKw => Some(TokenKind::IsDefVarKw),
            Self::TypeOfKw => Some(TokenKind::TypeOfKw),
            Self::PrintKw => Some(TokenKind::PrintKw),
            Self::DebugKw => Some(TokenKind::DebugKw),
            Self::EvalKw => Some(TokenKind::EvalKw),
            Self::OpenParen => Some(TokenKind::OpenParen),
            Self::CloseParen => Some(TokenKind::CloseParen),
            Self::OpenBrace => Some(TokenKind::OpenBrace),
            Self::CloseBrace => Some(TokenKind::CloseBrace),
            Self::OpenBracket => Some(TokenKind::OpenBracket),
            Self::CloseBracket => Some(TokenKind::CloseBracket),
            Self::HashBraceOpen => Some(TokenKind::HashBraceOpen),
            Self::Comma => Some(TokenKind::Comma),
            Self::Semicolon => Some(TokenKind::Semicolon),
            Self::Colon => Some(TokenKind::Colon),
            Self::ColonColon => Some(TokenKind::ColonColon),
            Self::Dot => Some(TokenKind::Dot),
            Self::QuestionDot => Some(TokenKind::QuestionDot),
            Self::QuestionOpenBracket => Some(TokenKind::QuestionOpenBracket),
            Self::FatArrow => Some(TokenKind::FatArrow),
            Self::Eq => Some(TokenKind::Eq),
            Self::PlusEq => Some(TokenKind::PlusEq),
            Self::MinusEq => Some(TokenKind::MinusEq),
            Self::StarEq => Some(TokenKind::StarEq),
            Self::SlashEq => Some(TokenKind::SlashEq),
            Self::PercentEq => Some(TokenKind::PercentEq),
            Self::StarStarEq => Some(TokenKind::StarStarEq),
            Self::ShlEq => Some(TokenKind::ShlEq),
            Self::ShrEq => Some(TokenKind::ShrEq),
            Self::AmpEq => Some(TokenKind::AmpEq),
            Self::PipeEq => Some(TokenKind::PipeEq),
            Self::CaretEq => Some(TokenKind::CaretEq),
            Self::QuestionQuestionEq => Some(TokenKind::QuestionQuestionEq),
            Self::Plus => Some(TokenKind::Plus),
            Self::Minus => Some(TokenKind::Minus),
            Self::Star => Some(TokenKind::Star),
            Self::Slash => Some(TokenKind::Slash),
            Self::Percent => Some(TokenKind::Percent),
            Self::StarStar => Some(TokenKind::StarStar),
            Self::Shl => Some(TokenKind::Shl),
            Self::Shr => Some(TokenKind::Shr),
            Self::Amp => Some(TokenKind::Amp),
            Self::Pipe => Some(TokenKind::Pipe),
            Self::Caret => Some(TokenKind::Caret),
            Self::AmpAmp => Some(TokenKind::AmpAmp),
            Self::PipePipe => Some(TokenKind::PipePipe),
            Self::Bang => Some(TokenKind::Bang),
            Self::EqEq => Some(TokenKind::EqEq),
            Self::BangEq => Some(TokenKind::BangEq),
            Self::Gt => Some(TokenKind::Gt),
            Self::GtEq => Some(TokenKind::GtEq),
            Self::Lt => Some(TokenKind::Lt),
            Self::LtEq => Some(TokenKind::LtEq),
            Self::QuestionQuestion => Some(TokenKind::QuestionQuestion),
            Self::Range => Some(TokenKind::Range),
            Self::RangeEq => Some(TokenKind::RangeEq),
            Self::Unknown => Some(TokenKind::Unknown),
            _ => None,
        }
    }
}

impl Language for RhaiLanguage {
    type Kind = RhaiKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        RhaiKind::from_rowan(raw)
            .unwrap_or_else(|| panic!("invalid rowan syntax kind for RhaiLanguage: {}", raw.0))
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.to_rowan()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct SyntaxError {
    message: String,
    range: TextRange,
}

impl SyntaxError {
    pub fn new(message: impl Into<String>, range: TextRange) -> Self {
        Self {
            message: message.into(),
            range,
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn range(&self) -> TextRange {
        self.range
    }
}

#[derive(Debug, Clone)]
pub struct Parse {
    text: Arc<str>,
    trivia: TriviaStore,
    green: RowanGreenNode,
    errors: Vec<SyntaxError>,
}

impl Parse {
    pub(crate) fn new(text: Arc<str>, green: RowanGreenNode, errors: Vec<SyntaxError>) -> Self {
        let root = RowanSyntaxNode::new_root(green.clone());
        let trivia = TriviaStore::new(&text, &root);
        Self {
            text,
            trivia,
            green,
            errors,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn root(&self) -> RowanSyntaxNode {
        RowanSyntaxNode::new_root(self.green.clone())
    }

    pub fn trivia(&self) -> &TriviaStore {
        &self.trivia
    }

    pub fn errors(&self) -> &[SyntaxError] {
        &self.errors
    }

    pub fn debug_tree(&self) -> String {
        let mut out = String::new();
        self.write_syntax_node(&mut out, &self.root(), 0);
        out
    }

    pub fn debug_tree_compact(&self) -> String {
        let mut out = String::new();
        self.write_compact_syntax_node(&mut out, &self.root(), 0);
        out
    }

    fn write_syntax_node(&self, out: &mut String, node: &RowanSyntaxNode, indent: usize) {
        let Some(kind) = node.kind().syntax_kind() else {
            return;
        };
        push_indent(out, indent);
        push_range(out, node.text_range());
        out.push_str(&format!("{kind:?}\n"));

        for child in node.children_with_tokens() {
            match child {
                RowanNodeOrToken::Node(node) => self.write_syntax_node(out, &node, indent + 2),
                RowanNodeOrToken::Token(token) => {
                    let Some(kind) = token.kind().token_kind() else {
                        continue;
                    };
                    if kind.is_trivia() {
                        continue;
                    }
                    push_indent(out, indent + 2);
                    push_range(out, token.text_range());
                    out.push_str(&format!("{kind:?} {:?}\n", token.text()));
                }
            }
        }
    }

    fn write_compact_syntax_node(&self, out: &mut String, node: &RowanSyntaxNode, indent: usize) {
        let Some(kind) = node.kind().syntax_kind() else {
            return;
        };
        push_indent(out, indent);
        out.push_str(&format!("{kind:?}\n"));

        for child in node.children_with_tokens() {
            match child {
                RowanNodeOrToken::Node(node) => {
                    self.write_compact_syntax_node(out, &node, indent + 2)
                }
                RowanNodeOrToken::Token(token) => {
                    let Some(kind) = token.kind().token_kind() else {
                        continue;
                    };
                    if kind.is_trivia() {
                        continue;
                    }
                    push_indent(out, indent + 2);
                    out.push_str(&format!("{kind:?} {:?}\n", token.text()));
                }
            }
        }
    }
}

pub trait RowanSyntaxNodeExt {
    fn child_nodes(&self) -> impl Iterator<Item = RowanSyntaxNode>;
    fn direct_raw_tokens(&self) -> impl Iterator<Item = RowanSyntaxToken>;
    fn direct_significant_tokens(&self) -> impl Iterator<Item = RowanSyntaxToken>;
    fn raw_tokens(&self) -> impl Iterator<Item = RowanSyntaxToken>;
    fn significant_tokens(&self) -> impl Iterator<Item = RowanSyntaxToken>;
    fn first_significant_token(&self) -> Option<RowanSyntaxToken>;
    fn last_significant_token(&self) -> Option<RowanSyntaxToken>;
    fn significant_range(&self) -> TextRange;
    fn structural_range(&self) -> TextRange;
}

impl RowanSyntaxNodeExt for RowanSyntaxNode {
    fn child_nodes(&self) -> impl Iterator<Item = RowanSyntaxNode> {
        self.children()
    }

    fn direct_raw_tokens(&self) -> impl Iterator<Item = RowanSyntaxToken> {
        self.children_with_tokens()
            .filter_map(|element| element.into_token())
    }

    fn direct_significant_tokens(&self) -> impl Iterator<Item = RowanSyntaxToken> {
        self.direct_raw_tokens().filter(is_significant_rowan_token)
    }

    fn raw_tokens(&self) -> impl Iterator<Item = RowanSyntaxToken> {
        self.descendants_with_tokens()
            .filter_map(|element| element.into_token())
    }

    fn significant_tokens(&self) -> impl Iterator<Item = RowanSyntaxToken> {
        self.raw_tokens().filter(is_significant_rowan_token)
    }

    fn first_significant_token(&self) -> Option<RowanSyntaxToken> {
        self.significant_tokens().next()
    }

    fn last_significant_token(&self) -> Option<RowanSyntaxToken> {
        self.significant_tokens().last()
    }

    fn significant_range(&self) -> TextRange {
        match (
            self.first_significant_token(),
            self.last_significant_token(),
        ) {
            (Some(first), Some(last)) => {
                TextRange::new(first.text_range().start(), last.text_range().end())
            }
            _ => self.text_range(),
        }
    }

    fn structural_range(&self) -> TextRange {
        let end = self
            .last_significant_token()
            .map(|token| token.text_range().end())
            .unwrap_or_else(|| self.text_range().end());
        TextRange::new(
            self.text_range().start(),
            end.max(self.text_range().start()),
        )
    }
}

fn is_significant_rowan_token(token: &RowanSyntaxToken) -> bool {
    token
        .kind()
        .token_kind()
        .is_some_and(|kind| !kind.is_trivia())
}

pub(crate) fn empty_range(offset: TextSize) -> TextRange {
    TextRange::new(offset, offset)
}

pub(crate) fn text_size_of(text: &str) -> TextSize {
    TextSize::from(u32::try_from(text.len()).unwrap_or(u32::MAX))
}

fn push_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push(' ');
    }
}

fn push_range(out: &mut String, range: TextRange) {
    let start = u32::from(range.start());
    let end = u32::from(range.end());
    out.push_str(&format!("{start}..{end} "));
}
