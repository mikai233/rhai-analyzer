use crate::ast::{
    AliasClause, AstNode, BlockExpr, BreakStmt, CatchClause, ConstStmt, ContinueStmt, ExportStmt,
    Expr, ExprStmt, ImportStmt, LetStmt, ReturnStmt, ThrowStmt, TryStmt, child, token_by_kind,
};
use crate::{SyntaxKind, SyntaxNode, SyntaxToken, TokenKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stmt<'a> {
    Let(LetStmt<'a>),
    Const(ConstStmt<'a>),
    Import(ImportStmt<'a>),
    Export(ExportStmt<'a>),
    Break(BreakStmt<'a>),
    Continue(ContinueStmt<'a>),
    Return(ReturnStmt<'a>),
    Throw(ThrowStmt<'a>),
    Try(TryStmt<'a>),
    Expr(ExprStmt<'a>),
}

impl<'a> AstNode<'a> for Stmt<'a> {
    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::StmtLet
                | SyntaxKind::StmtConst
                | SyntaxKind::StmtImport
                | SyntaxKind::StmtExport
                | SyntaxKind::StmtBreak
                | SyntaxKind::StmtContinue
                | SyntaxKind::StmtReturn
                | SyntaxKind::StmtThrow
                | SyntaxKind::StmtTry
                | SyntaxKind::StmtExpr
        )
    }

    fn cast(node: &'a SyntaxNode) -> Option<Self> {
        match node.kind() {
            SyntaxKind::StmtLet => Some(Self::Let(LetStmt { syntax: node })),
            SyntaxKind::StmtConst => Some(Self::Const(ConstStmt { syntax: node })),
            SyntaxKind::StmtImport => Some(Self::Import(ImportStmt { syntax: node })),
            SyntaxKind::StmtExport => Some(Self::Export(ExportStmt { syntax: node })),
            SyntaxKind::StmtBreak => Some(Self::Break(BreakStmt { syntax: node })),
            SyntaxKind::StmtContinue => Some(Self::Continue(ContinueStmt { syntax: node })),
            SyntaxKind::StmtReturn => Some(Self::Return(ReturnStmt { syntax: node })),
            SyntaxKind::StmtThrow => Some(Self::Throw(ThrowStmt { syntax: node })),
            SyntaxKind::StmtTry => Some(Self::Try(TryStmt { syntax: node })),
            SyntaxKind::StmtExpr => Some(Self::Expr(ExprStmt { syntax: node })),
            _ => None,
        }
    }

    fn syntax(self) -> &'a SyntaxNode {
        match self {
            Self::Let(stmt) => stmt.syntax(),
            Self::Const(stmt) => stmt.syntax(),
            Self::Import(stmt) => stmt.syntax(),
            Self::Export(stmt) => stmt.syntax(),
            Self::Break(stmt) => stmt.syntax(),
            Self::Continue(stmt) => stmt.syntax(),
            Self::Return(stmt) => stmt.syntax(),
            Self::Throw(stmt) => stmt.syntax(),
            Self::Try(stmt) => stmt.syntax(),
            Self::Expr(stmt) => stmt.syntax(),
        }
    }
}

impl<'a> LetStmt<'a> {
    pub fn name_token(self) -> Option<SyntaxToken> {
        token_by_kind(self.syntax, TokenKind::Ident)
    }

    pub fn initializer(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }
}

impl<'a> ConstStmt<'a> {
    pub fn name_token(self) -> Option<SyntaxToken> {
        token_by_kind(self.syntax, TokenKind::Ident)
    }

    pub fn value(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }
}

impl<'a> ImportStmt<'a> {
    pub fn module(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }

    pub fn alias(self) -> Option<AliasClause<'a>> {
        child(self.syntax)
    }
}

impl<'a> ExportStmt<'a> {
    pub fn target(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }

    pub fn declaration(self) -> Option<Stmt<'a>> {
        child(self.syntax)
    }

    pub fn alias(self) -> Option<AliasClause<'a>> {
        child(self.syntax)
    }
}

impl<'a> BreakStmt<'a> {
    pub fn value(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }
}

impl<'a> ReturnStmt<'a> {
    pub fn value(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }
}

impl<'a> ThrowStmt<'a> {
    pub fn value(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }
}

impl<'a> TryStmt<'a> {
    pub fn body(self) -> Option<BlockExpr<'a>> {
        child(self.syntax)
    }

    pub fn catch_clause(self) -> Option<CatchClause<'a>> {
        child(self.syntax)
    }
}

impl<'a> ExprStmt<'a> {
    pub fn expr(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }

    pub fn has_semicolon(self) -> bool {
        token_by_kind(self.syntax, TokenKind::Semicolon).is_some()
    }
}
