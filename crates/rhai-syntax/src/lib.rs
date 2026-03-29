mod ast;
mod lexer;
mod parser;
mod syntax;

pub use ast::{
    AliasClause, ArgList, ArrayExpr, ArrayItemList, AssignExpr, AstChildren, AstNode, BinaryExpr,
    BlockExpr, BreakStmt, CallExpr, CatchClause, ClosureExpr, ClosureParamList, ConstStmt,
    ContinueStmt, DoCondition, DoExpr, ElseBranch, ErrorNode, ExportStmt, Expr, ExprStmt,
    FieldExpr, FnItem, ForBindings, ForExpr, IfExpr, ImportStmt, IndexExpr, InterpolatedStringExpr,
    InterpolationBody, Item, LetStmt, LiteralExpr, LoopExpr, NameExpr, ObjectExpr, ObjectField,
    ParamList, ParenExpr, PathExpr, ReturnStmt, Root, Stmt, StringInterpolation, StringPart,
    StringSegment, SwitchArm, SwitchExpr, SwitchPatternList, ThrowStmt, TryStmt, UnaryExpr,
    WhileExpr,
};
pub use lexer::{Lexed, lex_text};
pub use parser::parse_text;
pub use syntax::{
    Parse, SyntaxElement, SyntaxError, SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize,
    TokenKind,
};

#[cfg(test)]
mod tests;
