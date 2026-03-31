use crate::ast::{
    AliasClause, AstNode, BlockExpr, BreakStmt, CatchClause, ConstStmt, ContinueStmt, ExportStmt,
    Expr, ExprStmt, ImportStmt, LetStmt, ReturnStmt, ThrowStmt, TryStmt, child, token_by_kind,
};
use crate::{RowanSyntaxNode, RowanSyntaxToken, SyntaxKind, TokenKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Let(LetStmt),
    Const(ConstStmt),
    Import(ImportStmt),
    Export(ExportStmt),
    Break(BreakStmt),
    Continue(ContinueStmt),
    Return(ReturnStmt),
    Throw(ThrowStmt),
    Try(TryStmt),
    Expr(ExprStmt),
}

impl AstNode for Stmt {
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

    fn cast(node: RowanSyntaxNode) -> Option<Self> {
        match node.kind().syntax_kind()? {
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

    fn syntax(&self) -> RowanSyntaxNode {
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

impl LetStmt {
    pub fn name_token(&self) -> Option<RowanSyntaxToken> {
        token_by_kind(&self.syntax, TokenKind::Ident)
    }

    pub fn initializer(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl ConstStmt {
    pub fn name_token(&self) -> Option<RowanSyntaxToken> {
        token_by_kind(&self.syntax, TokenKind::Ident)
    }

    pub fn value(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl ImportStmt {
    pub fn module(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    pub fn alias(&self) -> Option<AliasClause> {
        child(&self.syntax)
    }
}

impl ExportStmt {
    pub fn target(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    pub fn declaration(&self) -> Option<Stmt> {
        child(&self.syntax)
    }

    pub fn alias(&self) -> Option<AliasClause> {
        child(&self.syntax)
    }
}

impl BreakStmt {
    pub fn value(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl ReturnStmt {
    pub fn value(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl ThrowStmt {
    pub fn value(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl TryStmt {
    pub fn body(&self) -> Option<BlockExpr> {
        child(&self.syntax)
    }

    pub fn catch_clause(&self) -> Option<CatchClause> {
        child(&self.syntax)
    }
}

impl ExprStmt {
    pub fn expr(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    pub fn has_semicolon(&self) -> bool {
        token_by_kind(&self.syntax, TokenKind::Semicolon).is_some()
    }
}
