use crate::ast::{
    ArgList, ArrayExpr, ArrayItemList, AssignExpr, AstChildren, AstNode, BinaryExpr, BlockExpr,
    BlockItemList, CallExpr, ClosureExpr, ClosureParamList, DoCondition, DoExpr, ElseBranch,
    ErrorNode, FieldExpr, ForBindings, ForExpr, IfExpr, IndexExpr, InterpolatedStringExpr,
    InterpolationBody, InterpolationItemList, Item, LiteralExpr, LoopExpr, NameExpr, ObjectExpr,
    ObjectField, ObjectFieldList, ParenExpr, PathExpr, StringInterpolation, StringPartList,
    StringSegment, SwitchArm, SwitchArmList, SwitchExpr, SwitchPatternList, UnaryExpr, WhileExpr,
    child, children, find_token, is_assignment_operator, is_binary_operator, is_binding_token,
    is_literal_token, is_name_like_token, is_prefix_operator, nth_child, token_by_kind,
    token_children,
};
use crate::{SyntaxKind, SyntaxNode, SyntaxToken, TokenKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Name(NameExpr),
    Literal(LiteralExpr),
    Array(ArrayExpr),
    Object(ObjectExpr),
    If(IfExpr),
    Switch(SwitchExpr),
    While(WhileExpr),
    Loop(LoopExpr),
    For(ForExpr),
    Do(DoExpr),
    Path(PathExpr),
    Closure(ClosureExpr),
    InterpolatedString(InterpolatedStringExpr),
    Unary(UnaryExpr),
    Binary(BinaryExpr),
    Assign(AssignExpr),
    Paren(ParenExpr),
    Call(CallExpr),
    Index(IndexExpr),
    Field(FieldExpr),
    Block(BlockExpr),
    Error(ErrorNode),
}

impl AstNode for Expr {
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

    fn cast(node: SyntaxNode) -> Option<Self> {
        match node.kind().syntax_kind()? {
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

    fn syntax(&self) -> SyntaxNode {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StringPart {
    Segment(StringSegment),
    Interpolation(StringInterpolation),
}

impl AstNode for StringPart {
    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::StringSegment | SyntaxKind::StringInterpolation
        )
    }

    fn cast(node: SyntaxNode) -> Option<Self> {
        match node.kind().syntax_kind()? {
            SyntaxKind::StringSegment => Some(Self::Segment(StringSegment { syntax: node })),
            SyntaxKind::StringInterpolation => {
                Some(Self::Interpolation(StringInterpolation { syntax: node }))
            }
            _ => None,
        }
    }

    fn syntax(&self) -> SyntaxNode {
        match self {
            Self::Segment(part) => part.syntax(),
            Self::Interpolation(part) => part.syntax(),
        }
    }
}

impl NameExpr {
    pub fn token(&self) -> Option<SyntaxToken> {
        find_token(&self.syntax, is_name_like_token)
    }
}

impl LiteralExpr {
    pub fn token(&self) -> Option<SyntaxToken> {
        find_token(&self.syntax, is_literal_token)
    }
}

impl ArrayExpr {
    pub fn items(&self) -> Option<ArrayItemList> {
        child(&self.syntax)
    }
}

impl ArrayItemList {
    pub fn exprs(&self) -> AstChildren<Expr> {
        children(&self.syntax)
    }
}

impl ObjectExpr {
    pub fn field_list(&self) -> Option<ObjectFieldList> {
        child(&self.syntax)
    }
}

impl ObjectFieldList {
    pub fn fields(&self) -> AstChildren<ObjectField> {
        children(&self.syntax)
    }
}

impl ObjectField {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        find_token(&self.syntax, |kind| {
            matches!(kind, TokenKind::Ident | TokenKind::String)
        })
    }

    pub fn value(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl IfExpr {
    pub fn condition(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    pub fn then_branch(&self) -> Option<BlockExpr> {
        child(&self.syntax)
    }

    pub fn else_branch(&self) -> Option<ElseBranch> {
        child(&self.syntax)
    }
}

impl ElseBranch {
    pub fn body(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl SwitchExpr {
    pub fn scrutinee(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    pub fn arm_list(&self) -> Option<SwitchArmList> {
        child(&self.syntax)
    }
}

impl SwitchArmList {
    pub fn arms(&self) -> AstChildren<SwitchArm> {
        children(&self.syntax)
    }
}

impl SwitchArm {
    pub fn patterns(&self) -> Option<SwitchPatternList> {
        child(&self.syntax)
    }

    pub fn value(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl SwitchPatternList {
    pub fn exprs(&self) -> AstChildren<Expr> {
        children(&self.syntax)
    }

    pub fn wildcard_token(&self) -> Option<SyntaxToken> {
        token_by_kind(&self.syntax, TokenKind::Underscore)
    }
}

impl WhileExpr {
    pub fn condition(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    pub fn body(&self) -> Option<BlockExpr> {
        child(&self.syntax)
    }
}

impl LoopExpr {
    pub fn body(&self) -> Option<BlockExpr> {
        child(&self.syntax)
    }
}

impl ForExpr {
    pub fn bindings(&self) -> Option<ForBindings> {
        child(&self.syntax)
    }

    pub fn iterable(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    pub fn body(&self) -> Option<BlockExpr> {
        child(&self.syntax)
    }
}

impl ForBindings {
    pub fn names(&self) -> impl Iterator<Item = SyntaxToken> {
        token_children(&self.syntax)
            .filter(|token| token.kind().token_kind().is_some_and(is_binding_token))
    }
}

impl DoExpr {
    pub fn body(&self) -> Option<BlockExpr> {
        child(&self.syntax)
    }

    pub fn condition(&self) -> Option<DoCondition> {
        child(&self.syntax)
    }
}

impl DoCondition {
    pub fn keyword_token(&self) -> Option<SyntaxToken> {
        find_token(&self.syntax, |kind| {
            matches!(kind, TokenKind::WhileKw | TokenKind::UntilKw)
        })
    }

    pub fn expr(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl PathExpr {
    pub fn base(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    pub fn segments(&self) -> impl Iterator<Item = SyntaxToken> {
        token_children(&self.syntax)
            .filter(|token| token.kind().token_kind().is_some_and(is_name_like_token))
    }
}

impl ClosureExpr {
    pub fn params(&self) -> Option<ClosureParamList> {
        child(&self.syntax)
    }

    pub fn body(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl InterpolatedStringExpr {
    pub fn part_list(&self) -> Option<StringPartList> {
        child(&self.syntax)
    }
}

impl StringPartList {
    pub fn parts(&self) -> AstChildren<StringPart> {
        children(&self.syntax)
    }
}

impl StringSegment {
    pub fn text_token(&self) -> Option<SyntaxToken> {
        token_by_kind(&self.syntax, TokenKind::StringText)
    }
}

impl StringInterpolation {
    pub fn body(&self) -> Option<InterpolationBody> {
        child(&self.syntax)
    }
}

impl InterpolationBody {
    pub fn item_list(&self) -> Option<InterpolationItemList> {
        child(&self.syntax)
    }
}

impl InterpolationItemList {
    pub fn items(&self) -> AstChildren<Item> {
        children(&self.syntax)
    }
}

impl UnaryExpr {
    pub fn operator_token(&self) -> Option<SyntaxToken> {
        find_token(&self.syntax, is_prefix_operator)
    }

    pub fn expr(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl BinaryExpr {
    pub fn lhs(&self) -> Option<Expr> {
        nth_child(&self.syntax, 0)
    }

    pub fn operator_token(&self) -> Option<SyntaxToken> {
        find_token(&self.syntax, is_binary_operator)
    }

    pub fn rhs(&self) -> Option<Expr> {
        nth_child(&self.syntax, 1)
    }
}

impl AssignExpr {
    pub fn lhs(&self) -> Option<Expr> {
        nth_child(&self.syntax, 0)
    }

    pub fn operator_token(&self) -> Option<SyntaxToken> {
        find_token(&self.syntax, is_assignment_operator)
    }

    pub fn rhs(&self) -> Option<Expr> {
        nth_child(&self.syntax, 1)
    }
}

impl ParenExpr {
    pub fn expr(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl CallExpr {
    pub fn callee(&self) -> Option<Expr> {
        nth_child(&self.syntax, 0)
    }

    pub fn uses_caller_scope(&self) -> bool {
        token_by_kind(&self.syntax, TokenKind::Bang).is_some()
    }

    pub fn args(&self) -> Option<ArgList> {
        child(&self.syntax)
    }
}

impl ArgList {
    pub fn args(&self) -> AstChildren<Expr> {
        children(&self.syntax)
    }
}

impl IndexExpr {
    pub fn receiver(&self) -> Option<Expr> {
        nth_child(&self.syntax, 0)
    }

    pub fn index(&self) -> Option<Expr> {
        nth_child(&self.syntax, 1)
    }
}

impl FieldExpr {
    pub fn receiver(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    pub fn name_token(&self) -> Option<SyntaxToken> {
        find_token(&self.syntax, is_name_like_token)
    }
}

impl BlockExpr {
    pub fn item_list(&self) -> Option<BlockItemList> {
        child(&self.syntax)
    }
}

impl BlockItemList {
    pub fn items(&self) -> AstChildren<Item> {
        children(&self.syntax)
    }
}
