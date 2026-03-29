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
    pub(crate) fn new(node: &'a SyntaxNode) -> Self {
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

mod expr;
mod item;
mod stmt;

pub use crate::ast::expr::{Expr, StringPart};
pub use crate::ast::item::Item;
pub use crate::ast::stmt::Stmt;

pub(crate) fn children<'a, N>(node: &'a SyntaxNode) -> AstChildren<'a, N>
where
    N: AstNode<'a>,
{
    AstChildren::new(node)
}

pub(crate) fn child<'a, N>(node: &'a SyntaxNode) -> Option<N>
where
    N: AstNode<'a>,
{
    children(node).next()
}

pub(crate) fn nth_child<'a, N>(node: &'a SyntaxNode, index: usize) -> Option<N>
where
    N: AstNode<'a>,
{
    children(node).nth(index)
}

pub(crate) fn token_children(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> + '_ {
    node.children().iter().filter_map(SyntaxElement::as_token)
}

pub(crate) fn token_by_kind(node: &SyntaxNode, kind: TokenKind) -> Option<SyntaxToken> {
    token_children(node).find(|token| token.kind() == kind)
}

pub(crate) fn find_token<F>(node: &SyntaxNode, mut predicate: F) -> Option<SyntaxToken>
where
    F: FnMut(TokenKind) -> bool,
{
    token_children(node).find(|token| predicate(token.kind()))
}

pub(crate) fn is_param_token(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Ident | TokenKind::Underscore | TokenKind::ThisKw
    )
}

pub(crate) fn is_binding_token(kind: TokenKind) -> bool {
    matches!(kind, TokenKind::Ident | TokenKind::Underscore)
}

pub(crate) fn is_name_like_token(kind: TokenKind) -> bool {
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

pub(crate) fn is_literal_token(kind: TokenKind) -> bool {
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

pub(crate) fn is_prefix_operator(kind: TokenKind) -> bool {
    matches!(kind, TokenKind::Plus | TokenKind::Minus | TokenKind::Bang)
}

pub(crate) fn is_binary_operator(kind: TokenKind) -> bool {
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

pub(crate) fn is_assignment_operator(kind: TokenKind) -> bool {
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
