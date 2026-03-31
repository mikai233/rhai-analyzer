use rhai_syntax::Expr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FormatSupportLevel {
    Full,
    Structural,
    RawFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SyntaxFamily {
    Name,
    Literal,
    Array,
    Object,
    If,
    Switch,
    While,
    Loop,
    For,
    Do,
    Path,
    Closure,
    InterpolatedString,
    Unary,
    Binary,
    Assign,
    Paren,
    Call,
    Index,
    Field,
    Block,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FormatSupport {
    pub(crate) family: SyntaxFamily,
    pub(crate) level: FormatSupportLevel,
}

pub(crate) fn expr_support(expr: Expr<'_>) -> FormatSupport {
    match expr {
        Expr::Name(_) => support(SyntaxFamily::Name, FormatSupportLevel::Full),
        Expr::Literal(_) => support(SyntaxFamily::Literal, FormatSupportLevel::Full),
        Expr::Array(_) => support(SyntaxFamily::Array, FormatSupportLevel::Full),
        Expr::Object(_) => support(SyntaxFamily::Object, FormatSupportLevel::Full),
        Expr::If(_) => support(SyntaxFamily::If, FormatSupportLevel::Structural),
        Expr::Switch(_) => support(SyntaxFamily::Switch, FormatSupportLevel::Structural),
        Expr::While(_) => support(SyntaxFamily::While, FormatSupportLevel::Structural),
        Expr::Loop(_) => support(SyntaxFamily::Loop, FormatSupportLevel::Structural),
        Expr::For(_) => support(SyntaxFamily::For, FormatSupportLevel::Structural),
        Expr::Do(_) => support(SyntaxFamily::Do, FormatSupportLevel::Structural),
        Expr::Path(_) => support(SyntaxFamily::Path, FormatSupportLevel::Structural),
        Expr::Closure(_) => support(SyntaxFamily::Closure, FormatSupportLevel::Structural),
        Expr::InterpolatedString(_) => support(
            SyntaxFamily::InterpolatedString,
            FormatSupportLevel::Structural,
        ),
        Expr::Unary(_) => support(SyntaxFamily::Unary, FormatSupportLevel::Structural),
        Expr::Binary(_) => support(SyntaxFamily::Binary, FormatSupportLevel::Structural),
        Expr::Assign(_) => support(SyntaxFamily::Assign, FormatSupportLevel::Structural),
        Expr::Paren(_) => support(SyntaxFamily::Paren, FormatSupportLevel::Structural),
        Expr::Call(_) => support(SyntaxFamily::Call, FormatSupportLevel::Structural),
        Expr::Index(_) => support(SyntaxFamily::Index, FormatSupportLevel::Structural),
        Expr::Field(_) => support(SyntaxFamily::Field, FormatSupportLevel::Structural),
        Expr::Block(_) => support(SyntaxFamily::Block, FormatSupportLevel::Full),
        Expr::Error(_) => support(SyntaxFamily::Error, FormatSupportLevel::RawFallback),
    }
}

fn support(family: SyntaxFamily, level: FormatSupportLevel) -> FormatSupport {
    FormatSupport { family, level }
}
