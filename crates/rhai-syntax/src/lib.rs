mod ast;
mod lexer;
mod parser;
mod syntax;
mod trivia;

pub use ast::{
    AliasClause, ArgList, ArrayExpr, ArrayItemList, AssignExpr, AstChildren, AstNode, BinaryExpr,
    BlockExpr, BlockItemList, BreakStmt, CallExpr, CatchClause, ClosureExpr, ClosureParamList,
    ConstStmt, ContinueStmt, DoCondition, DoExpr, ElseBranch, ErrorNode, ExportStmt, Expr,
    ExprStmt, FieldExpr, FnItem, ForBindings, ForExpr, IfExpr, ImportStmt, IndexExpr,
    InterpolatedStringExpr, InterpolationBody, InterpolationItemList, Item, LetStmt, LiteralExpr,
    LoopExpr, NameExpr, ObjectExpr, ObjectField, ObjectFieldList, ParamList, ParenExpr, PathExpr,
    ReturnStmt, Root, RootItemList, Stmt, StringInterpolation, StringPart, StringPartList,
    StringSegment, SwitchArm, SwitchArmList, SwitchExpr, SwitchPatternList, ThrowStmt, TryStmt,
    UnaryExpr, WhileExpr,
};
pub use lexer::{Lexed, lex_text};
pub use parser::parse_text;
pub use syntax::{
    GreenNode, GreenToken, LexToken, NodeOrToken, Parse, RawSyntaxKind, RhaiKind, RhaiLanguage,
    SyntaxElement, SyntaxError, SyntaxKind, SyntaxNode, SyntaxNodeExt, SyntaxToken, TextRange,
    TextSize, TokenKind,
};
pub use trivia::{
    AttachedComment, CommentKind, GapTrivia, TriviaBoundary, TriviaSlot, TriviaStore,
};

#[cfg(test)]
mod tests;
