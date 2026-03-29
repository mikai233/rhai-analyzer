use std::{marker::PhantomData, slice};

use crate::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken, TokenKind};

pub trait AstNode<'a>: Copy + Sized {
    fn can_cast(kind: SyntaxKind) -> bool;
    fn cast(node: &'a SyntaxNode) -> Option<Self>;
    fn syntax(self) -> &'a SyntaxNode;
}

#[derive(Debug, Clone)]
pub struct AstChildren<'a, N> {
    inner: slice::Iter<'a, SyntaxElement>,
    marker: PhantomData<N>,
}

impl<'a, N> AstChildren<'a, N> {
    fn new(node: &'a SyntaxNode) -> Self {
        Self {
            inner: node.children().iter(),
            marker: PhantomData,
        }
    }
}

impl<'a, N> Iterator for AstChildren<'a, N>
where
    N: AstNode<'a>,
{
    type Item = N;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .find_map(|element| element.as_node().and_then(N::cast))
    }
}

macro_rules! simple_ast_node {
    ($name:ident, $kind:path) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name<'a> {
            syntax: &'a SyntaxNode,
        }

        impl<'a> AstNode<'a> for $name<'a> {
            fn can_cast(kind: SyntaxKind) -> bool {
                kind == $kind
            }

            fn cast(node: &'a SyntaxNode) -> Option<Self> {
                Self::can_cast(node.kind()).then_some(Self { syntax: node })
            }

            fn syntax(self) -> &'a SyntaxNode {
                self.syntax
            }
        }
    };
}

simple_ast_node!(Root, SyntaxKind::Root);
simple_ast_node!(FnItem, SyntaxKind::ItemFn);
simple_ast_node!(LetStmt, SyntaxKind::StmtLet);
simple_ast_node!(ConstStmt, SyntaxKind::StmtConst);
simple_ast_node!(ImportStmt, SyntaxKind::StmtImport);
simple_ast_node!(ExportStmt, SyntaxKind::StmtExport);
simple_ast_node!(BreakStmt, SyntaxKind::StmtBreak);
simple_ast_node!(ContinueStmt, SyntaxKind::StmtContinue);
simple_ast_node!(ReturnStmt, SyntaxKind::StmtReturn);
simple_ast_node!(ThrowStmt, SyntaxKind::StmtThrow);
simple_ast_node!(TryStmt, SyntaxKind::StmtTry);
simple_ast_node!(ExprStmt, SyntaxKind::StmtExpr);
simple_ast_node!(NameExpr, SyntaxKind::ExprName);
simple_ast_node!(LiteralExpr, SyntaxKind::ExprLiteral);
simple_ast_node!(ArrayExpr, SyntaxKind::ExprArray);
simple_ast_node!(ObjectExpr, SyntaxKind::ExprObject);
simple_ast_node!(IfExpr, SyntaxKind::ExprIf);
simple_ast_node!(SwitchExpr, SyntaxKind::ExprSwitch);
simple_ast_node!(WhileExpr, SyntaxKind::ExprWhile);
simple_ast_node!(LoopExpr, SyntaxKind::ExprLoop);
simple_ast_node!(ForExpr, SyntaxKind::ExprFor);
simple_ast_node!(DoExpr, SyntaxKind::ExprDo);
simple_ast_node!(PathExpr, SyntaxKind::ExprPath);
simple_ast_node!(ClosureExpr, SyntaxKind::ExprClosure);
simple_ast_node!(InterpolatedStringExpr, SyntaxKind::ExprInterpolatedString);
simple_ast_node!(UnaryExpr, SyntaxKind::ExprUnary);
simple_ast_node!(BinaryExpr, SyntaxKind::ExprBinary);
simple_ast_node!(AssignExpr, SyntaxKind::ExprAssign);
simple_ast_node!(ParenExpr, SyntaxKind::ExprParen);
simple_ast_node!(CallExpr, SyntaxKind::ExprCall);
simple_ast_node!(IndexExpr, SyntaxKind::ExprIndex);
simple_ast_node!(FieldExpr, SyntaxKind::ExprField);
simple_ast_node!(ArgList, SyntaxKind::ArgList);
simple_ast_node!(ParamList, SyntaxKind::ParamList);
simple_ast_node!(ClosureParamList, SyntaxKind::ClosureParamList);
simple_ast_node!(ArrayItemList, SyntaxKind::ArrayItemList);
simple_ast_node!(ObjectField, SyntaxKind::ObjectField);
simple_ast_node!(StringSegment, SyntaxKind::StringSegment);
simple_ast_node!(StringInterpolation, SyntaxKind::StringInterpolation);
simple_ast_node!(InterpolationBody, SyntaxKind::InterpolationBody);
simple_ast_node!(ElseBranch, SyntaxKind::ElseBranch);
simple_ast_node!(ForBindings, SyntaxKind::ForBindings);
simple_ast_node!(AliasClause, SyntaxKind::AliasClause);
simple_ast_node!(SwitchArm, SyntaxKind::SwitchArm);
simple_ast_node!(SwitchPatternList, SyntaxKind::SwitchPatternList);
simple_ast_node!(DoCondition, SyntaxKind::DoCondition);
simple_ast_node!(CatchClause, SyntaxKind::CatchClause);
simple_ast_node!(BlockExpr, SyntaxKind::Block);
simple_ast_node!(ErrorNode, SyntaxKind::Error);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item<'a> {
    Fn(FnItem<'a>),
    Stmt(Stmt<'a>),
}

impl<'a> AstNode<'a> for Item<'a> {
    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::ItemFn
                | SyntaxKind::StmtLet
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
            SyntaxKind::ItemFn => Some(Self::Fn(FnItem { syntax: node })),
            _ => Stmt::cast(node).map(Self::Stmt),
        }
    }

    fn syntax(self) -> &'a SyntaxNode {
        match self {
            Self::Fn(item) => item.syntax(),
            Self::Stmt(stmt) => stmt.syntax(),
        }
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expr<'a> {
    Name(NameExpr<'a>),
    Literal(LiteralExpr<'a>),
    Array(ArrayExpr<'a>),
    Object(ObjectExpr<'a>),
    If(IfExpr<'a>),
    Switch(SwitchExpr<'a>),
    While(WhileExpr<'a>),
    Loop(LoopExpr<'a>),
    For(ForExpr<'a>),
    Do(DoExpr<'a>),
    Path(PathExpr<'a>),
    Closure(ClosureExpr<'a>),
    InterpolatedString(InterpolatedStringExpr<'a>),
    Unary(UnaryExpr<'a>),
    Binary(BinaryExpr<'a>),
    Assign(AssignExpr<'a>),
    Paren(ParenExpr<'a>),
    Call(CallExpr<'a>),
    Index(IndexExpr<'a>),
    Field(FieldExpr<'a>),
    Block(BlockExpr<'a>),
    Error(ErrorNode<'a>),
}

impl<'a> AstNode<'a> for Expr<'a> {
    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::ExprName
                | SyntaxKind::ExprLiteral
                | SyntaxKind::ExprArray
                | SyntaxKind::ExprObject
                | SyntaxKind::ExprIf
                | SyntaxKind::ExprSwitch
                | SyntaxKind::ExprWhile
                | SyntaxKind::ExprLoop
                | SyntaxKind::ExprFor
                | SyntaxKind::ExprDo
                | SyntaxKind::ExprPath
                | SyntaxKind::ExprClosure
                | SyntaxKind::ExprInterpolatedString
                | SyntaxKind::ExprUnary
                | SyntaxKind::ExprBinary
                | SyntaxKind::ExprAssign
                | SyntaxKind::ExprParen
                | SyntaxKind::ExprCall
                | SyntaxKind::ExprIndex
                | SyntaxKind::ExprField
                | SyntaxKind::Block
                | SyntaxKind::Error
        )
    }

    fn cast(node: &'a SyntaxNode) -> Option<Self> {
        match node.kind() {
            SyntaxKind::ExprName => Some(Self::Name(NameExpr { syntax: node })),
            SyntaxKind::ExprLiteral => Some(Self::Literal(LiteralExpr { syntax: node })),
            SyntaxKind::ExprArray => Some(Self::Array(ArrayExpr { syntax: node })),
            SyntaxKind::ExprObject => Some(Self::Object(ObjectExpr { syntax: node })),
            SyntaxKind::ExprIf => Some(Self::If(IfExpr { syntax: node })),
            SyntaxKind::ExprSwitch => Some(Self::Switch(SwitchExpr { syntax: node })),
            SyntaxKind::ExprWhile => Some(Self::While(WhileExpr { syntax: node })),
            SyntaxKind::ExprLoop => Some(Self::Loop(LoopExpr { syntax: node })),
            SyntaxKind::ExprFor => Some(Self::For(ForExpr { syntax: node })),
            SyntaxKind::ExprDo => Some(Self::Do(DoExpr { syntax: node })),
            SyntaxKind::ExprPath => Some(Self::Path(PathExpr { syntax: node })),
            SyntaxKind::ExprClosure => Some(Self::Closure(ClosureExpr { syntax: node })),
            SyntaxKind::ExprInterpolatedString => {
                Some(Self::InterpolatedString(InterpolatedStringExpr {
                    syntax: node,
                }))
            }
            SyntaxKind::ExprUnary => Some(Self::Unary(UnaryExpr { syntax: node })),
            SyntaxKind::ExprBinary => Some(Self::Binary(BinaryExpr { syntax: node })),
            SyntaxKind::ExprAssign => Some(Self::Assign(AssignExpr { syntax: node })),
            SyntaxKind::ExprParen => Some(Self::Paren(ParenExpr { syntax: node })),
            SyntaxKind::ExprCall => Some(Self::Call(CallExpr { syntax: node })),
            SyntaxKind::ExprIndex => Some(Self::Index(IndexExpr { syntax: node })),
            SyntaxKind::ExprField => Some(Self::Field(FieldExpr { syntax: node })),
            SyntaxKind::Block => Some(Self::Block(BlockExpr { syntax: node })),
            SyntaxKind::Error => Some(Self::Error(ErrorNode { syntax: node })),
            _ => None,
        }
    }

    fn syntax(self) -> &'a SyntaxNode {
        match self {
            Self::Name(expr) => expr.syntax(),
            Self::Literal(expr) => expr.syntax(),
            Self::Array(expr) => expr.syntax(),
            Self::Object(expr) => expr.syntax(),
            Self::If(expr) => expr.syntax(),
            Self::Switch(expr) => expr.syntax(),
            Self::While(expr) => expr.syntax(),
            Self::Loop(expr) => expr.syntax(),
            Self::For(expr) => expr.syntax(),
            Self::Do(expr) => expr.syntax(),
            Self::Path(expr) => expr.syntax(),
            Self::Closure(expr) => expr.syntax(),
            Self::InterpolatedString(expr) => expr.syntax(),
            Self::Unary(expr) => expr.syntax(),
            Self::Binary(expr) => expr.syntax(),
            Self::Assign(expr) => expr.syntax(),
            Self::Paren(expr) => expr.syntax(),
            Self::Call(expr) => expr.syntax(),
            Self::Index(expr) => expr.syntax(),
            Self::Field(expr) => expr.syntax(),
            Self::Block(expr) => expr.syntax(),
            Self::Error(expr) => expr.syntax(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringPart<'a> {
    Segment(StringSegment<'a>),
    Interpolation(StringInterpolation<'a>),
}

impl<'a> AstNode<'a> for StringPart<'a> {
    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::StringSegment | SyntaxKind::StringInterpolation
        )
    }

    fn cast(node: &'a SyntaxNode) -> Option<Self> {
        match node.kind() {
            SyntaxKind::StringSegment => Some(Self::Segment(StringSegment { syntax: node })),
            SyntaxKind::StringInterpolation => {
                Some(Self::Interpolation(StringInterpolation { syntax: node }))
            }
            _ => None,
        }
    }

    fn syntax(self) -> &'a SyntaxNode {
        match self {
            Self::Segment(part) => part.syntax(),
            Self::Interpolation(part) => part.syntax(),
        }
    }
}

impl<'a> Root<'a> {
    pub fn items(self) -> AstChildren<'a, Item<'a>> {
        children(self.syntax)
    }
}

impl<'a> FnItem<'a> {
    pub fn is_private(self) -> bool {
        token_by_kind(self.syntax, TokenKind::PrivateKw).is_some()
    }

    pub fn name_token(self) -> Option<SyntaxToken> {
        token_by_kind(self.syntax, TokenKind::Ident)
    }

    pub fn params(self) -> Option<ParamList<'a>> {
        child(self.syntax)
    }

    pub fn body(self) -> Option<BlockExpr<'a>> {
        child(self.syntax)
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

impl<'a> NameExpr<'a> {
    pub fn token(self) -> Option<SyntaxToken> {
        find_token(self.syntax, is_name_like_token)
    }
}

impl<'a> LiteralExpr<'a> {
    pub fn token(self) -> Option<SyntaxToken> {
        find_token(self.syntax, is_literal_token)
    }
}

impl<'a> ArrayExpr<'a> {
    pub fn items(self) -> Option<ArrayItemList<'a>> {
        child(self.syntax)
    }
}

impl<'a> ArrayItemList<'a> {
    pub fn exprs(self) -> AstChildren<'a, Expr<'a>> {
        children(self.syntax)
    }
}

impl<'a> ObjectExpr<'a> {
    pub fn fields(self) -> AstChildren<'a, ObjectField<'a>> {
        children(self.syntax)
    }
}

impl<'a> ObjectField<'a> {
    pub fn name_token(self) -> Option<SyntaxToken> {
        find_token(self.syntax, |kind| {
            matches!(kind, TokenKind::Ident | TokenKind::String)
        })
    }

    pub fn value(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }
}

impl<'a> IfExpr<'a> {
    pub fn condition(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }

    pub fn then_branch(self) -> Option<BlockExpr<'a>> {
        child(self.syntax)
    }

    pub fn else_branch(self) -> Option<ElseBranch<'a>> {
        child(self.syntax)
    }
}

impl<'a> ElseBranch<'a> {
    pub fn body(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }
}

impl<'a> SwitchExpr<'a> {
    pub fn scrutinee(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }

    pub fn arms(self) -> AstChildren<'a, SwitchArm<'a>> {
        children(self.syntax)
    }
}

impl<'a> SwitchArm<'a> {
    pub fn patterns(self) -> Option<SwitchPatternList<'a>> {
        child(self.syntax)
    }

    pub fn value(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }
}

impl<'a> SwitchPatternList<'a> {
    pub fn exprs(self) -> AstChildren<'a, Expr<'a>> {
        children(self.syntax)
    }

    pub fn wildcard_token(self) -> Option<SyntaxToken> {
        token_by_kind(self.syntax, TokenKind::Underscore)
    }
}

impl<'a> WhileExpr<'a> {
    pub fn condition(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }

    pub fn body(self) -> Option<BlockExpr<'a>> {
        child(self.syntax)
    }
}

impl<'a> LoopExpr<'a> {
    pub fn body(self) -> Option<BlockExpr<'a>> {
        child(self.syntax)
    }
}

impl<'a> ForExpr<'a> {
    pub fn bindings(self) -> Option<ForBindings<'a>> {
        child(self.syntax)
    }

    pub fn iterable(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }

    pub fn body(self) -> Option<BlockExpr<'a>> {
        child(self.syntax)
    }
}

impl<'a> ForBindings<'a> {
    pub fn names(self) -> impl Iterator<Item = SyntaxToken> + 'a {
        token_children(self.syntax).filter(|token| is_binding_token(token.kind()))
    }
}

impl<'a> DoExpr<'a> {
    pub fn body(self) -> Option<BlockExpr<'a>> {
        child(self.syntax)
    }

    pub fn condition(self) -> Option<DoCondition<'a>> {
        child(self.syntax)
    }
}

impl<'a> DoCondition<'a> {
    pub fn keyword_token(self) -> Option<SyntaxToken> {
        find_token(self.syntax, |kind| {
            matches!(kind, TokenKind::WhileKw | TokenKind::UntilKw)
        })
    }

    pub fn expr(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }
}

impl<'a> PathExpr<'a> {
    pub fn base(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }

    pub fn segments(self) -> impl Iterator<Item = SyntaxToken> + 'a {
        token_children(self.syntax).filter(|token| is_name_like_token(token.kind()))
    }
}

impl<'a> ClosureExpr<'a> {
    pub fn params(self) -> Option<ClosureParamList<'a>> {
        child(self.syntax)
    }

    pub fn body(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }
}

impl<'a> InterpolatedStringExpr<'a> {
    pub fn parts(self) -> AstChildren<'a, StringPart<'a>> {
        children(self.syntax)
    }
}

impl<'a> StringSegment<'a> {
    pub fn text_token(self) -> Option<SyntaxToken> {
        token_by_kind(self.syntax, TokenKind::StringText)
    }
}

impl<'a> StringInterpolation<'a> {
    pub fn body(self) -> Option<InterpolationBody<'a>> {
        child(self.syntax)
    }
}

impl<'a> InterpolationBody<'a> {
    pub fn items(self) -> AstChildren<'a, Item<'a>> {
        children(self.syntax)
    }
}

impl<'a> UnaryExpr<'a> {
    pub fn operator_token(self) -> Option<SyntaxToken> {
        find_token(self.syntax, is_prefix_operator)
    }

    pub fn expr(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }
}

impl<'a> BinaryExpr<'a> {
    pub fn lhs(self) -> Option<Expr<'a>> {
        nth_child(self.syntax, 0)
    }

    pub fn operator_token(self) -> Option<SyntaxToken> {
        find_token(self.syntax, is_binary_operator)
    }

    pub fn rhs(self) -> Option<Expr<'a>> {
        nth_child(self.syntax, 1)
    }
}

impl<'a> AssignExpr<'a> {
    pub fn lhs(self) -> Option<Expr<'a>> {
        nth_child(self.syntax, 0)
    }

    pub fn operator_token(self) -> Option<SyntaxToken> {
        find_token(self.syntax, is_assignment_operator)
    }

    pub fn rhs(self) -> Option<Expr<'a>> {
        nth_child(self.syntax, 1)
    }
}

impl<'a> ParenExpr<'a> {
    pub fn expr(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }
}

impl<'a> CallExpr<'a> {
    pub fn callee(self) -> Option<Expr<'a>> {
        nth_child(self.syntax, 0)
    }

    pub fn args(self) -> Option<ArgList<'a>> {
        child(self.syntax)
    }
}

impl<'a> ArgList<'a> {
    pub fn args(self) -> AstChildren<'a, Expr<'a>> {
        children(self.syntax)
    }
}

impl<'a> IndexExpr<'a> {
    pub fn receiver(self) -> Option<Expr<'a>> {
        nth_child(self.syntax, 0)
    }

    pub fn index(self) -> Option<Expr<'a>> {
        nth_child(self.syntax, 1)
    }
}

impl<'a> FieldExpr<'a> {
    pub fn receiver(self) -> Option<Expr<'a>> {
        child(self.syntax)
    }

    pub fn name_token(self) -> Option<SyntaxToken> {
        find_token(self.syntax, is_name_like_token)
    }
}

impl<'a> ParamList<'a> {
    pub fn params(self) -> impl Iterator<Item = SyntaxToken> + 'a {
        token_children(self.syntax).filter(|token| is_param_token(token.kind()))
    }
}

impl<'a> ClosureParamList<'a> {
    pub fn params(self) -> impl Iterator<Item = SyntaxToken> + 'a {
        token_children(self.syntax).filter(|token| is_param_token(token.kind()))
    }
}

impl<'a> AliasClause<'a> {
    pub fn alias_token(self) -> Option<SyntaxToken> {
        find_token(self.syntax, |kind| {
            matches!(kind, TokenKind::Ident | TokenKind::GlobalKw)
        })
    }
}

impl<'a> CatchClause<'a> {
    pub fn binding_token(self) -> Option<SyntaxToken> {
        find_token(self.syntax, is_binding_token)
    }

    pub fn body(self) -> Option<BlockExpr<'a>> {
        child(self.syntax)
    }
}

impl<'a> BlockExpr<'a> {
    pub fn items(self) -> AstChildren<'a, Item<'a>> {
        children(self.syntax)
    }
}

fn children<'a, N>(node: &'a SyntaxNode) -> AstChildren<'a, N>
where
    N: AstNode<'a>,
{
    AstChildren::new(node)
}

fn child<'a, N>(node: &'a SyntaxNode) -> Option<N>
where
    N: AstNode<'a>,
{
    children(node).next()
}

fn nth_child<'a, N>(node: &'a SyntaxNode, index: usize) -> Option<N>
where
    N: AstNode<'a>,
{
    children(node).nth(index)
}

fn token_children(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> + '_ {
    node.children().iter().filter_map(SyntaxElement::as_token)
}

fn token_by_kind(node: &SyntaxNode, kind: TokenKind) -> Option<SyntaxToken> {
    token_children(node).find(|token| token.kind() == kind)
}

fn find_token<F>(node: &SyntaxNode, mut predicate: F) -> Option<SyntaxToken>
where
    F: FnMut(TokenKind) -> bool,
{
    token_children(node).find(|token| predicate(token.kind()))
}

fn is_param_token(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Ident | TokenKind::Underscore | TokenKind::ThisKw
    )
}

fn is_binding_token(kind: TokenKind) -> bool {
    matches!(kind, TokenKind::Ident | TokenKind::Underscore)
}

fn is_name_like_token(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Ident
            | TokenKind::ThisKw
            | TokenKind::GlobalKw
            | TokenKind::FnPtrKw
            | TokenKind::CallKw
            | TokenKind::CurryKw
            | TokenKind::IsSharedKw
            | TokenKind::IsDefFnKw
            | TokenKind::IsDefVarKw
            | TokenKind::TypeOfKw
            | TokenKind::PrintKw
            | TokenKind::DebugKw
            | TokenKind::EvalKw
    )
}

fn is_literal_token(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Int
            | TokenKind::Float
            | TokenKind::String
            | TokenKind::RawString
            | TokenKind::BacktickString
            | TokenKind::Char
            | TokenKind::TrueKw
            | TokenKind::FalseKw
    )
}

fn is_prefix_operator(kind: TokenKind) -> bool {
    matches!(kind, TokenKind::Plus | TokenKind::Minus | TokenKind::Bang)
}

fn is_binary_operator(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::PipePipe
            | TokenKind::Pipe
            | TokenKind::Caret
            | TokenKind::AmpAmp
            | TokenKind::Amp
            | TokenKind::EqEq
            | TokenKind::BangEq
            | TokenKind::InKw
            | TokenKind::Gt
            | TokenKind::GtEq
            | TokenKind::Lt
            | TokenKind::LtEq
            | TokenKind::QuestionQuestion
            | TokenKind::Range
            | TokenKind::RangeEq
            | TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Percent
            | TokenKind::StarStar
            | TokenKind::Shl
            | TokenKind::Shr
    )
}

fn is_assignment_operator(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Eq
            | TokenKind::PlusEq
            | TokenKind::MinusEq
            | TokenKind::StarEq
            | TokenKind::SlashEq
            | TokenKind::PercentEq
            | TokenKind::StarStarEq
            | TokenKind::ShlEq
            | TokenKind::ShrEq
            | TokenKind::AmpEq
            | TokenKind::PipeEq
            | TokenKind::CaretEq
            | TokenKind::QuestionQuestionEq
    )
}

#[cfg(test)]
mod tests {
    use crate::parse_text;

    use super::{AstNode, Expr, Item, Root, Stmt, StringPart};

    #[test]
    fn typed_wrappers_expose_root_items_and_function_shape() {
        let parse = parse_text(
            r#"
            private fn add(x, y) { return x + y; }
            let value = add(1, 2);
        "#,
        );

        let root = Root::cast(parse.root()).expect("root should cast");
        let mut items = root.items();

        let Item::Fn(function) = items.next().expect("expected function item") else {
            panic!("expected function item");
        };
        assert!(function.is_private());
        assert_eq!(
            function
                .name_token()
                .expect("expected function name")
                .text(parse.text()),
            "add"
        );

        let params: Vec<_> = function
            .params()
            .expect("expected parameter list")
            .params()
            .map(|token| token.text(parse.text()))
            .collect();
        assert_eq!(params, vec!["x", "y"]);

        let body = function.body().expect("expected function body");
        let Item::Stmt(Stmt::Return(return_stmt)) = body.items().next().expect("expected return")
        else {
            panic!("expected return statement");
        };
        let Expr::Binary(binary) = return_stmt.value().expect("expected return value") else {
            panic!("expected binary expression");
        };
        assert_eq!(
            binary
                .operator_token()
                .expect("expected operator token")
                .text(parse.text()),
            "+"
        );

        let Item::Stmt(Stmt::Let(let_stmt)) = items.next().expect("expected let statement") else {
            panic!("expected let statement");
        };
        assert_eq!(
            let_stmt
                .name_token()
                .expect("expected binding name")
                .text(parse.text()),
            "value"
        );
    }

    #[test]
    fn typed_wrappers_expose_closure_and_call_structure() {
        let parse = parse_text("let add = |x, y| x + y; let value = add(1, 2 * 3);");
        let root = Root::cast(parse.root()).expect("root should cast");
        let mut items = root.items();

        let Item::Stmt(Stmt::Let(add_stmt)) = items.next().expect("expected let statement") else {
            panic!("expected let statement");
        };
        let Expr::Closure(closure) = add_stmt.initializer().expect("expected closure") else {
            panic!("expected closure expression");
        };
        let closure_params: Vec<_> = closure
            .params()
            .expect("expected closure parameter list")
            .params()
            .map(|token| token.text(parse.text()))
            .collect();
        assert_eq!(closure_params, vec!["x", "y"]);

        let Expr::Binary(closure_body) = closure.body().expect("expected closure body") else {
            panic!("expected binary closure body");
        };
        assert_eq!(
            closure_body
                .operator_token()
                .expect("expected closure operator")
                .text(parse.text()),
            "+"
        );

        let Item::Stmt(Stmt::Let(value_stmt)) = items.next().expect("expected let statement")
        else {
            panic!("expected let statement");
        };
        let Expr::Call(call) = value_stmt.initializer().expect("expected call") else {
            panic!("expected call expression");
        };
        let Expr::Name(callee) = call.callee().expect("expected callee") else {
            panic!("expected name callee");
        };
        assert_eq!(
            callee
                .token()
                .expect("expected callee token")
                .text(parse.text()),
            "add"
        );

        let args: Vec<_> = call
            .args()
            .expect("expected argument list")
            .args()
            .collect();
        assert_eq!(args.len(), 2);
        assert!(matches!(args[0], Expr::Literal(_)));
        assert!(matches!(args[1], Expr::Binary(_)));
    }

    #[test]
    fn typed_wrappers_expose_interpolated_string_parts() {
        let parse = parse_text(r#"let msg = `value=${x + 1}`;"#);
        let root = Root::cast(parse.root()).expect("root should cast");

        let Item::Stmt(Stmt::Let(let_stmt)) = root.items().next().expect("expected let statement")
        else {
            panic!("expected let statement");
        };
        let Expr::InterpolatedString(interpolated) = let_stmt
            .initializer()
            .expect("expected interpolated string")
        else {
            panic!("expected interpolated string expression");
        };

        let parts: Vec<_> = interpolated.parts().collect();
        assert_eq!(parts.len(), 2);

        let StringPart::Segment(segment) = parts[0] else {
            panic!("expected string segment");
        };
        assert_eq!(
            segment
                .text_token()
                .expect("expected string text")
                .text(parse.text()),
            "value="
        );

        let StringPart::Interpolation(interpolation) = parts[1] else {
            panic!("expected interpolation part");
        };
        let body = interpolation.body().expect("expected interpolation body");
        let expr_stmt = match body.items().next().expect("expected expression statement") {
            Item::Stmt(Stmt::Expr(stmt)) => stmt,
            _ => panic!("expected expression statement"),
        };

        let Expr::Binary(_) = expr_stmt.expr().expect("expected binary expr") else {
            panic!("expected binary expression");
        };
    }

    #[test]
    fn typed_wrappers_expose_const_import_export_and_aliases() {
        let parse = parse_text(
            r#"
            const ANSWER = 42;
            import "crypto" as secure;
            export global::crypto::sha256 as exported;
        "#,
        );
        let root = Root::cast(parse.root()).expect("root should cast");
        let mut items = root.items();

        let Item::Stmt(Stmt::Const(const_stmt)) = items.next().expect("expected const statement")
        else {
            panic!("expected const statement");
        };
        assert_eq!(
            const_stmt
                .name_token()
                .expect("expected const name")
                .text(parse.text()),
            "ANSWER"
        );
        let Expr::Literal(value) = const_stmt.value().expect("expected const value") else {
            panic!("expected literal const value");
        };
        assert_eq!(
            value
                .token()
                .expect("expected literal token")
                .text(parse.text()),
            "42"
        );

        let Item::Stmt(Stmt::Import(import_stmt)) =
            items.next().expect("expected import statement")
        else {
            panic!("expected import statement");
        };
        let Expr::Literal(module) = import_stmt.module().expect("expected import module") else {
            panic!("expected import literal");
        };
        assert_eq!(
            module
                .token()
                .expect("expected import literal token")
                .text(parse.text()),
            r#""crypto""#
        );
        assert_eq!(
            import_stmt
                .alias()
                .expect("expected import alias")
                .alias_token()
                .expect("expected import alias token")
                .text(parse.text()),
            "secure"
        );

        let Item::Stmt(Stmt::Export(export_stmt)) =
            items.next().expect("expected export statement")
        else {
            panic!("expected export statement");
        };
        let Expr::Path(path) = export_stmt.target().expect("expected export target") else {
            panic!("expected export path");
        };
        let Expr::Name(base) = path.base().expect("expected path base") else {
            panic!("expected path base name");
        };
        assert_eq!(
            base.token()
                .expect("expected path base token")
                .text(parse.text()),
            "global"
        );
        let segments: Vec<_> = path
            .segments()
            .map(|token| token.text(parse.text()))
            .collect();
        assert_eq!(segments, vec!["crypto", "sha256"]);
        assert_eq!(
            export_stmt
                .alias()
                .expect("expected export alias")
                .alias_token()
                .expect("expected export alias token")
                .text(parse.text()),
            "exported"
        );
    }

    #[test]
    fn typed_wrappers_expose_collections_and_access_chains() {
        let parse = parse_text(r#"let value = (#{ name: "rhai", data: [1, 2, 3] }.data)[1];"#);
        let root = Root::cast(parse.root()).expect("root should cast");

        let Item::Stmt(Stmt::Let(let_stmt)) = root.items().next().expect("expected let statement")
        else {
            panic!("expected let statement");
        };
        let Expr::Index(index) = let_stmt.initializer().expect("expected index expr") else {
            panic!("expected index expression");
        };

        let Expr::Paren(paren) = index.receiver().expect("expected parenthesized receiver") else {
            panic!("expected paren receiver");
        };
        let Expr::Field(field) = paren.expr().expect("expected field expression") else {
            panic!("expected field expression");
        };
        assert_eq!(
            field
                .name_token()
                .expect("expected field name token")
                .text(parse.text()),
            "data"
        );

        let Expr::Object(object) = field.receiver().expect("expected object receiver") else {
            panic!("expected object receiver");
        };
        let fields: Vec<_> = object.fields().collect();
        assert_eq!(fields.len(), 2);
        assert_eq!(
            fields[0]
                .name_token()
                .expect("expected object field name")
                .text(parse.text()),
            "name"
        );
        let Expr::Literal(name_value) = fields[0].value().expect("expected object field value")
        else {
            panic!("expected literal object field value");
        };
        assert_eq!(
            name_value
                .token()
                .expect("expected object literal token")
                .text(parse.text()),
            r#""rhai""#
        );

        let Expr::Array(array) = fields[1].value().expect("expected array field value") else {
            panic!("expected array field value");
        };
        let items: Vec<_> = array
            .items()
            .expect("expected array item list")
            .exprs()
            .collect();
        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|expr| matches!(expr, Expr::Literal(_))));

        let Expr::Literal(index_expr) = index.index().expect("expected index expression") else {
            panic!("expected literal index");
        };
        assert_eq!(
            index_expr
                .token()
                .expect("expected index token")
                .text(parse.text()),
            "1"
        );
    }

    #[test]
    fn typed_wrappers_expose_control_flow_nodes() {
        let parse = parse_text(
            r#"
            let decision = if flag { 1 } else { 2 };
            let switched = switch mode { 0 => 1, _ => { 2 } };
            while cond { continue; }
            loop { break 1; }
            for (item, index) in items { item; }
            do { value; } until done;
        "#,
        );
        let root = Root::cast(parse.root()).expect("root should cast");
        let mut items = root.items();

        let Item::Stmt(Stmt::Let(decision_stmt)) = items.next().expect("expected let statement")
        else {
            panic!("expected let statement");
        };
        let Expr::If(if_expr) = decision_stmt.initializer().expect("expected if expr") else {
            panic!("expected if expression");
        };
        let Expr::Name(condition) = if_expr.condition().expect("expected if condition") else {
            panic!("expected if condition name");
        };
        assert_eq!(
            condition
                .token()
                .expect("expected if condition token")
                .text(parse.text()),
            "flag"
        );
        let Item::Stmt(Stmt::Expr(then_stmt)) = if_expr
            .then_branch()
            .expect("expected then block")
            .items()
            .next()
            .expect("expected then stmt")
        else {
            panic!("expected then expr stmt");
        };
        assert!(matches!(then_stmt.expr(), Some(Expr::Literal(_))));
        let Expr::Block(else_block) = if_expr
            .else_branch()
            .expect("expected else branch")
            .body()
            .expect("expected else body")
        else {
            panic!("expected else block");
        };
        assert_eq!(else_block.items().count(), 1);

        let Item::Stmt(Stmt::Let(switched_stmt)) = items.next().expect("expected let statement")
        else {
            panic!("expected let statement");
        };
        let Expr::Switch(switch_expr) = switched_stmt.initializer().expect("expected switch expr")
        else {
            panic!("expected switch expression");
        };
        let Expr::Name(scrutinee) = switch_expr.scrutinee().expect("expected scrutinee") else {
            panic!("expected switch scrutinee name");
        };
        assert_eq!(
            scrutinee
                .token()
                .expect("expected scrutinee token")
                .text(parse.text()),
            "mode"
        );
        let arms: Vec<_> = switch_expr.arms().collect();
        assert_eq!(arms.len(), 2);
        let first_patterns: Vec<_> = arms[0]
            .patterns()
            .expect("expected switch patterns")
            .exprs()
            .collect();
        assert_eq!(first_patterns.len(), 1);
        assert!(
            arms[1]
                .patterns()
                .expect("expected wildcard patterns")
                .wildcard_token()
                .is_some()
        );
        assert!(matches!(
            arms[1].value().expect("expected wildcard arm value"),
            Expr::Block(_)
        ));

        let Item::Stmt(Stmt::Expr(while_stmt)) = items.next().expect("expected while stmt") else {
            panic!("expected while expr stmt");
        };
        let Expr::While(while_expr) = while_stmt.expr().expect("expected while expr") else {
            panic!("expected while expression");
        };
        let Expr::Name(while_condition) = while_expr.condition().expect("expected while condition")
        else {
            panic!("expected while condition name");
        };
        assert_eq!(
            while_condition
                .token()
                .expect("expected while condition token")
                .text(parse.text()),
            "cond"
        );
        assert!(matches!(
            while_expr
                .body()
                .expect("expected while body")
                .items()
                .next(),
            Some(Item::Stmt(Stmt::Continue(_)))
        ));

        let Item::Stmt(Stmt::Expr(loop_stmt)) = items.next().expect("expected loop stmt") else {
            panic!("expected loop expr stmt");
        };
        let Expr::Loop(loop_expr) = loop_stmt.expr().expect("expected loop expr") else {
            panic!("expected loop expression");
        };
        let Item::Stmt(Stmt::Break(break_stmt)) = loop_expr
            .body()
            .expect("expected loop body")
            .items()
            .next()
            .expect("expected break stmt")
        else {
            panic!("expected break statement");
        };
        assert!(matches!(break_stmt.value(), Some(Expr::Literal(_))));

        let Item::Stmt(Stmt::Expr(for_stmt)) = items.next().expect("expected for stmt") else {
            panic!("expected for expr stmt");
        };
        let Expr::For(for_expr) = for_stmt.expr().expect("expected for expr") else {
            panic!("expected for expression");
        };
        let binding_names: Vec<_> = for_expr
            .bindings()
            .expect("expected for bindings")
            .names()
            .map(|token| token.text(parse.text()))
            .collect();
        assert_eq!(binding_names, vec!["item", "index"]);
        assert!(matches!(for_expr.iterable(), Some(Expr::Name(_))));
        assert_eq!(
            for_expr.body().expect("expected for body").items().count(),
            1
        );

        let Item::Stmt(Stmt::Expr(do_stmt)) = items.next().expect("expected do stmt") else {
            panic!("expected do expr stmt");
        };
        let Expr::Do(do_expr) = do_stmt.expr().expect("expected do expr") else {
            panic!("expected do expression");
        };
        assert_eq!(do_expr.body().expect("expected do body").items().count(), 1);
        let condition = do_expr.condition().expect("expected do condition");
        assert_eq!(
            condition
                .keyword_token()
                .expect("expected do condition keyword")
                .text(parse.text()),
            "until"
        );
        assert!(matches!(condition.expr(), Some(Expr::Name(_))));
    }

    #[test]
    fn typed_wrappers_expose_try_assignment_and_error_nodes() {
        let parse = parse_text(
            r#"
            try { throw err; } catch (error) { return global::module::call; }
            value = -(input);
            let broken = ;
        "#,
        );
        let root = Root::cast(parse.root()).expect("root should cast");
        let mut items = root.items();

        let Item::Stmt(Stmt::Try(try_stmt)) = items.next().expect("expected try stmt") else {
            panic!("expected try statement");
        };
        let Item::Stmt(Stmt::Throw(throw_stmt)) = try_stmt
            .body()
            .expect("expected try body")
            .items()
            .next()
            .expect("expected throw stmt")
        else {
            panic!("expected throw statement");
        };
        assert!(matches!(throw_stmt.value(), Some(Expr::Name(_))));

        let catch_clause = try_stmt.catch_clause().expect("expected catch clause");
        assert_eq!(
            catch_clause
                .binding_token()
                .expect("expected catch binding")
                .text(parse.text()),
            "error"
        );
        let Item::Stmt(Stmt::Return(return_stmt)) = catch_clause
            .body()
            .expect("expected catch body")
            .items()
            .next()
            .expect("expected return stmt")
        else {
            panic!("expected return statement");
        };
        let Expr::Path(path) = return_stmt.value().expect("expected return path") else {
            panic!("expected path return value");
        };
        let Expr::Name(base) = path.base().expect("expected path base") else {
            panic!("expected path base name");
        };
        assert_eq!(
            base.token()
                .expect("expected path base token")
                .text(parse.text()),
            "global"
        );
        let segments: Vec<_> = path
            .segments()
            .map(|token| token.text(parse.text()))
            .collect();
        assert_eq!(segments, vec!["module", "call"]);

        let Item::Stmt(Stmt::Expr(assign_stmt)) = items.next().expect("expected expr stmt") else {
            panic!("expected expression statement");
        };
        let Expr::Assign(assign) = assign_stmt.expr().expect("expected assignment expr") else {
            panic!("expected assignment expression");
        };
        assert!(matches!(assign.lhs(), Some(Expr::Name(_))));
        assert_eq!(
            assign
                .operator_token()
                .expect("expected assignment operator")
                .text(parse.text()),
            "="
        );
        let Expr::Unary(unary) = assign.rhs().expect("expected unary rhs") else {
            panic!("expected unary rhs");
        };
        assert_eq!(
            unary
                .operator_token()
                .expect("expected unary operator")
                .text(parse.text()),
            "-"
        );
        let Expr::Paren(paren) = unary.expr().expect("expected paren operand") else {
            panic!("expected paren operand");
        };
        assert!(matches!(paren.expr(), Some(Expr::Name(_))));

        let Item::Stmt(Stmt::Let(broken_stmt)) = items.next().expect("expected broken let stmt")
        else {
            panic!("expected broken let statement");
        };
        assert!(matches!(broken_stmt.initializer(), Some(Expr::Error(_))));
    }
}
