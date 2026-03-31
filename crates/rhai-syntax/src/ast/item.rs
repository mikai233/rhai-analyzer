use crate::ast::{
    AliasClause, AstChildren, AstNode, BlockExpr, CatchClause, ClosureParamList, FnItem, ParamList,
    Root, RootItemList, Stmt, child, children, find_token, is_binding_token, is_param_token,
    token_by_kind, token_children,
};
use crate::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken, TokenKind};

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

impl<'a> Root<'a> {
    pub fn item_list(self) -> Option<RootItemList<'a>> {
        child(self.syntax)
    }
}

impl<'a> RootItemList<'a> {
    pub fn items(self) -> AstChildren<'a, Item<'a>> {
        children(self.syntax)
    }
}

impl<'a> FnItem<'a> {
    pub fn is_private(self) -> bool {
        token_by_kind(self.syntax, TokenKind::PrivateKw).is_some()
    }

    pub fn name_token(self) -> Option<SyntaxToken> {
        let mut last_ident = None;
        for element in self.syntax.significant_children() {
            match element {
                SyntaxElement::Node(node) if node.kind() == SyntaxKind::ParamList => break,
                SyntaxElement::Token(token) if token.kind() == TokenKind::Ident => {
                    last_ident = Some(*token);
                }
                _ => {}
            }
        }
        last_ident
    }

    pub fn is_typed_method(self) -> bool {
        token_by_kind(self.syntax, TokenKind::Dot).is_some()
    }

    pub fn this_type_token(self) -> Option<SyntaxToken> {
        if !self.is_typed_method() {
            return None;
        }

        let mut saw_fn = false;
        for element in self.syntax.significant_children() {
            match element {
                SyntaxElement::Token(token) if token.kind() == TokenKind::FnKw => {
                    saw_fn = true;
                }
                SyntaxElement::Token(token)
                    if saw_fn && matches!(token.kind(), TokenKind::Ident | TokenKind::String) =>
                {
                    return Some(*token);
                }
                SyntaxElement::Node(node) if node.kind() == SyntaxKind::ParamList => break,
                _ => {}
            }
        }

        None
    }

    pub fn this_type_name(self, source: &str) -> Option<String> {
        let token = self.this_type_token()?;
        let text = token.text(source);
        match token.kind() {
            TokenKind::Ident => Some(text.to_owned()),
            TokenKind::String if text.len() >= 2 => Some(text[1..text.len() - 1].to_owned()),
            _ => None,
        }
    }

    pub fn params(self) -> Option<ParamList<'a>> {
        child(self.syntax)
    }

    pub fn body(self) -> Option<BlockExpr<'a>> {
        child(self.syntax)
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
