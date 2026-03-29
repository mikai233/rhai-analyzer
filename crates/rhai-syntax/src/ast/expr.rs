use crate::ast::{
    ArgList, ArrayExpr, ArrayItemList, AssignExpr, AstChildren, AstNode, BinaryExpr, BlockExpr,
    CallExpr, ClosureExpr, ClosureParamList, DoCondition, DoExpr, ElseBranch, ErrorNode, FieldExpr,
    ForBindings, ForExpr, IfExpr, IndexExpr, InterpolatedStringExpr, InterpolationBody, Item,
    LiteralExpr, LoopExpr, NameExpr, ObjectExpr, ObjectField, ParenExpr, PathExpr,
    StringInterpolation, StringSegment, SwitchArm, SwitchExpr, SwitchPatternList, UnaryExpr,
    WhileExpr, child, children, find_token, is_assignment_operator, is_binary_operator,
    is_binding_token, is_literal_token, is_name_like_token, is_prefix_operator, nth_child,
    token_by_kind, token_children,
};
use crate::{SyntaxKind, SyntaxNode, SyntaxToken, TokenKind};

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

    pub fn uses_caller_scope(self) -> bool {
        token_by_kind(self.syntax, TokenKind::Bang).is_some()
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

impl<'a> BlockExpr<'a> {
    pub fn items(self) -> AstChildren<'a, Item<'a>> {
        children(self.syntax)
    }
}
