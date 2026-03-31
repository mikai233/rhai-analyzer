use rhai_syntax::{Expr, Item, Stmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FormatSupportLevel {
    Full,
    Structural,
    RawFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExprFamily {
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
pub(crate) enum StmtFamily {
    Let,
    Const,
    Import,
    Export,
    Break,
    Continue,
    Return,
    Throw,
    Try,
    Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ItemFamily {
    Function,
    Statement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FormatSupport<Family> {
    pub(crate) family: Family,
    pub(crate) level: FormatSupportLevel,
}

pub(crate) fn expr_support(expr: Expr<'_>) -> FormatSupport<ExprFamily> {
    match expr {
        Expr::Name(_) => support(ExprFamily::Name, FormatSupportLevel::Full),
        Expr::Literal(_) => support(ExprFamily::Literal, FormatSupportLevel::Full),
        Expr::Array(_) => support(ExprFamily::Array, FormatSupportLevel::Full),
        Expr::Object(_) => support(ExprFamily::Object, FormatSupportLevel::Full),
        Expr::If(_) => support(ExprFamily::If, FormatSupportLevel::Structural),
        Expr::Switch(_) => support(ExprFamily::Switch, FormatSupportLevel::Structural),
        Expr::While(_) => support(ExprFamily::While, FormatSupportLevel::Structural),
        Expr::Loop(_) => support(ExprFamily::Loop, FormatSupportLevel::Structural),
        Expr::For(_) => support(ExprFamily::For, FormatSupportLevel::Structural),
        Expr::Do(_) => support(ExprFamily::Do, FormatSupportLevel::Structural),
        Expr::Path(_) => support(ExprFamily::Path, FormatSupportLevel::Structural),
        Expr::Closure(_) => support(ExprFamily::Closure, FormatSupportLevel::Structural),
        Expr::InterpolatedString(_) => support(
            ExprFamily::InterpolatedString,
            FormatSupportLevel::Structural,
        ),
        Expr::Unary(_) => support(ExprFamily::Unary, FormatSupportLevel::Structural),
        Expr::Binary(_) => support(ExprFamily::Binary, FormatSupportLevel::Structural),
        Expr::Assign(_) => support(ExprFamily::Assign, FormatSupportLevel::Structural),
        Expr::Paren(_) => support(ExprFamily::Paren, FormatSupportLevel::Structural),
        Expr::Call(_) => support(ExprFamily::Call, FormatSupportLevel::Structural),
        Expr::Index(_) => support(ExprFamily::Index, FormatSupportLevel::Structural),
        Expr::Field(_) => support(ExprFamily::Field, FormatSupportLevel::Structural),
        Expr::Block(_) => support(ExprFamily::Block, FormatSupportLevel::Full),
        Expr::Error(_) => support(ExprFamily::Error, FormatSupportLevel::RawFallback),
    }
}

pub(crate) fn stmt_support(stmt: Stmt<'_>) -> FormatSupport<StmtFamily> {
    match stmt {
        Stmt::Let(_) => support(StmtFamily::Let, FormatSupportLevel::Structural),
        Stmt::Const(_) => support(StmtFamily::Const, FormatSupportLevel::Structural),
        Stmt::Import(_) => support(StmtFamily::Import, FormatSupportLevel::Structural),
        Stmt::Export(_) => support(StmtFamily::Export, FormatSupportLevel::Structural),
        Stmt::Break(_) => support(StmtFamily::Break, FormatSupportLevel::Structural),
        Stmt::Continue(_) => support(StmtFamily::Continue, FormatSupportLevel::Structural),
        Stmt::Return(_) => support(StmtFamily::Return, FormatSupportLevel::Structural),
        Stmt::Throw(_) => support(StmtFamily::Throw, FormatSupportLevel::Structural),
        Stmt::Try(_) => support(StmtFamily::Try, FormatSupportLevel::Structural),
        Stmt::Expr(_) => support(StmtFamily::Expr, FormatSupportLevel::Structural),
    }
}

pub(crate) fn item_support(item: Item<'_>) -> FormatSupport<ItemFamily> {
    match item {
        Item::Fn(_) => support(ItemFamily::Function, FormatSupportLevel::Structural),
        Item::Stmt(_) => support(ItemFamily::Statement, FormatSupportLevel::Structural),
    }
}

fn support<Family>(family: Family, level: FormatSupportLevel) -> FormatSupport<Family> {
    FormatSupport { family, level }
}
