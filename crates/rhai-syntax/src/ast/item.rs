use crate::ast::{
    AliasClause, AstChildren, AstNode, BlockExpr, CatchClause, ClosureParamList, FnItem, ParamList,
    Root, RootItemList, Stmt, child, children, find_token, is_binding_token, is_param_token,
    token_by_kind, token_children,
};
use crate::{RowanSyntaxNode, RowanSyntaxToken, SyntaxKind, TokenKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Fn(FnItem),
    Stmt(Stmt),
}

impl AstNode for Item {
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

    fn cast(node: RowanSyntaxNode) -> Option<Self> {
        match node.kind().syntax_kind()? {
            SyntaxKind::ItemFn => Some(Self::Fn(FnItem { syntax: node })),
            _ => Stmt::cast(node).map(Self::Stmt),
        }
    }

    fn syntax(&self) -> RowanSyntaxNode {
        match self {
            Self::Fn(item) => item.syntax(),
            Self::Stmt(stmt) => stmt.syntax(),
        }
    }
}

impl Root {
    pub fn item_list(&self) -> Option<RootItemList> {
        child(&self.syntax)
    }
}

impl RootItemList {
    pub fn items(&self) -> AstChildren<Item> {
        children(&self.syntax)
    }
}

impl FnItem {
    pub fn is_private(&self) -> bool {
        token_by_kind(&self.syntax, TokenKind::PrivateKw).is_some()
    }

    pub fn name_token(&self) -> Option<RowanSyntaxToken> {
        let mut last_ident = None;
        for element in self.syntax.children_with_tokens() {
            match element {
                rowan::NodeOrToken::Node(node)
                    if node.kind().syntax_kind() == Some(SyntaxKind::ParamList) =>
                {
                    break;
                }
                rowan::NodeOrToken::Token(token)
                    if token.kind().token_kind() == Some(TokenKind::Ident) =>
                {
                    last_ident = Some(token);
                }
                _ => {}
            }
        }
        last_ident
    }

    pub fn is_typed_method(&self) -> bool {
        token_by_kind(&self.syntax, TokenKind::Dot).is_some()
    }

    pub fn this_type_token(&self) -> Option<RowanSyntaxToken> {
        if !self.clone().is_typed_method() {
            return None;
        }

        let syntax = self.syntax.clone();
        let mut saw_fn = false;
        for element in syntax.children_with_tokens() {
            match element {
                rowan::NodeOrToken::Token(token)
                    if token.kind().token_kind() == Some(TokenKind::FnKw) =>
                {
                    saw_fn = true;
                }
                rowan::NodeOrToken::Token(token)
                    if saw_fn
                        && matches!(
                            token.kind().token_kind(),
                            Some(TokenKind::Ident | TokenKind::String)
                        ) =>
                {
                    return Some(token);
                }
                rowan::NodeOrToken::Node(node)
                    if node.kind().syntax_kind() == Some(SyntaxKind::ParamList) =>
                {
                    break;
                }
                _ => {}
            }
        }

        None
    }

    pub fn this_type_name(&self) -> Option<String> {
        let token = self.this_type_token()?;
        let text = token.text().to_string();
        match token.kind().token_kind() {
            Some(TokenKind::Ident) => Some(text),
            Some(TokenKind::String) if text.len() >= 2 => Some(text[1..text.len() - 1].to_owned()),
            _ => None,
        }
    }

    pub fn params(&self) -> Option<ParamList> {
        child(&self.syntax)
    }

    pub fn body(&self) -> Option<BlockExpr> {
        child(&self.syntax)
    }
}

impl ParamList {
    pub fn params(&self) -> impl Iterator<Item = RowanSyntaxToken> {
        let syntax = self.syntax.clone();
        token_children(&syntax)
            .filter(|token| token.kind().token_kind().is_some_and(is_param_token))
            .collect::<Vec<_>>()
            .into_iter()
    }
}

impl ClosureParamList {
    pub fn params(&self) -> impl Iterator<Item = RowanSyntaxToken> {
        let syntax = self.syntax.clone();
        token_children(&syntax)
            .filter(|token| token.kind().token_kind().is_some_and(is_param_token))
            .collect::<Vec<_>>()
            .into_iter()
    }
}

impl AliasClause {
    pub fn alias_token(&self) -> Option<RowanSyntaxToken> {
        find_token(&self.syntax, |kind| {
            matches!(kind, TokenKind::Ident | TokenKind::GlobalKw)
        })
    }
}

impl CatchClause {
    pub fn binding_token(&self) -> Option<RowanSyntaxToken> {
        find_token(&self.syntax, is_binding_token)
    }

    pub fn body(&self) -> Option<BlockExpr> {
        child(&self.syntax)
    }
}
