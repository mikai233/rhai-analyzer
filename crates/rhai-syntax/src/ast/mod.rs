use std::marker::PhantomData;

use crate::{SyntaxKind, SyntaxNode, SyntaxToken, TokenKind};

pub trait AstNode: Clone + Sized {
    fn can_cast(kind: SyntaxKind) -> bool;
    fn cast(node: SyntaxNode) -> Option<Self>;
    fn syntax(&self) -> SyntaxNode;
}

#[derive(Debug, Clone)]
pub struct AstChildren<N> {
    inner: rowan::SyntaxNodeChildren<crate::RhaiLanguage>,
    marker: PhantomData<N>,
}

impl<N> AstChildren<N> {
    pub(crate) fn new(node: &SyntaxNode) -> Self {
        Self {
            inner: node.children(),
            marker: PhantomData,
        }
    }
}

impl<N> Iterator for AstChildren<N>
where
    N: AstNode,
{
    type Item = N;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.find_map(N::cast)
    }
}

macro_rules! simple_ast_node {
    ($name:ident, $kind:path) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name {
            pub(crate) syntax: SyntaxNode,
        }

        impl AstNode for $name {
            fn can_cast(kind: SyntaxKind) -> bool {
                kind == $kind
            }

            fn cast(node: SyntaxNode) -> Option<Self> {
                (node.kind().syntax_kind() == Some($kind)).then_some(Self { syntax: node })
            }

            fn syntax(&self) -> SyntaxNode {
                self.syntax.clone()
            }
        }
    };
}

simple_ast_node!(Root, SyntaxKind::Root);
simple_ast_node!(RootItemList, SyntaxKind::RootItemList);
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
simple_ast_node!(ObjectFieldList, SyntaxKind::ObjectFieldList);
simple_ast_node!(ObjectField, SyntaxKind::ObjectField);
simple_ast_node!(SwitchArmList, SyntaxKind::SwitchArmList);
simple_ast_node!(StringPartList, SyntaxKind::StringPartList);
simple_ast_node!(StringSegment, SyntaxKind::StringSegment);
simple_ast_node!(StringInterpolation, SyntaxKind::StringInterpolation);
simple_ast_node!(InterpolationBody, SyntaxKind::InterpolationBody);
simple_ast_node!(InterpolationItemList, SyntaxKind::InterpolationItemList);
simple_ast_node!(ElseBranch, SyntaxKind::ElseBranch);
simple_ast_node!(ForBindings, SyntaxKind::ForBindings);
simple_ast_node!(AliasClause, SyntaxKind::AliasClause);
simple_ast_node!(SwitchArm, SyntaxKind::SwitchArm);
simple_ast_node!(SwitchPatternList, SyntaxKind::SwitchPatternList);
simple_ast_node!(DoCondition, SyntaxKind::DoCondition);
simple_ast_node!(CatchClause, SyntaxKind::CatchClause);
simple_ast_node!(BlockExpr, SyntaxKind::Block);
simple_ast_node!(BlockItemList, SyntaxKind::BlockItemList);
simple_ast_node!(ErrorNode, SyntaxKind::Error);

mod expr;
mod item;
mod stmt;

pub use crate::ast::expr::{Expr, StringPart};
pub use crate::ast::item::Item;
pub use crate::ast::stmt::Stmt;

pub(crate) fn children<N>(node: &SyntaxNode) -> AstChildren<N>
where
    N: AstNode,
{
    AstChildren::new(node)
}

pub(crate) fn child<N>(node: &SyntaxNode) -> Option<N>
where
    N: AstNode,
{
    children(node).next()
}

pub(crate) fn nth_child<N>(node: &SyntaxNode, index: usize) -> Option<N>
where
    N: AstNode,
{
    children(node).nth(index)
}

pub(crate) fn token_children(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> + '_ {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| {
            token
                .kind()
                .token_kind()
                .is_some_and(|kind| !kind.is_trivia())
        })
}

pub(crate) fn token_by_kind(node: &SyntaxNode, kind: TokenKind) -> Option<SyntaxToken> {
    token_children(node).find(|token| token.kind().token_kind() == Some(kind))
}

pub(crate) fn find_token<F>(node: &SyntaxNode, mut predicate: F) -> Option<SyntaxToken>
where
    F: FnMut(TokenKind) -> bool,
{
    token_children(node).find(|token| token.kind().token_kind().is_some_and(&mut predicate))
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
