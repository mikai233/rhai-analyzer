use crate::ast::{
    AliasClause, AstChildren, AstNode, BlockExpr, CatchClause, ClosureParamList, FnItem, ParamList,
    Root, RootItemList, Stmt, child, children, find_token, is_binding_token, is_param_token,
    token_by_kind, token_children,
};
use crate::{SyntaxKind, SyntaxNode, SyntaxToken, TokenKind};

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

    fn cast(node: SyntaxNode) -> Option<Self> {
        match node.kind().syntax_kind()? {
            SyntaxKind::ItemFn => Some(Self::Fn(FnItem { syntax: node })),
            _ => Stmt::cast(node).map(Self::Stmt),
        }
    }

    fn syntax(&self) -> SyntaxNode {
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
    pub fn signature_tokens(&self) -> impl Iterator<Item = SyntaxToken> {
        self.syntax
            .children_with_tokens()
            .take_while(|element| {
                !matches!(
                    element,
                    rowan::NodeOrToken::Node(node)
                        if node.kind().syntax_kind() == Some(SyntaxKind::ParamList)
                )
            })
            .filter_map(|element| match element {
                rowan::NodeOrToken::Token(token)
                    if token
                        .kind()
                        .token_kind()
                        .is_some_and(|kind| !kind.is_trivia()) =>
                {
                    Some(token)
                }
                _ => None,
            })
    }

    pub fn is_private(&self) -> bool {
        token_by_kind(&self.syntax, TokenKind::PrivateKw).is_some()
    }

    pub fn name_token(&self) -> Option<SyntaxToken> {
        self.signature_tokens()
            .filter(|token| token.kind().token_kind() == Some(TokenKind::Ident))
            .last()
    }

    pub fn is_typed_method(&self) -> bool {
        token_by_kind(&self.syntax, TokenKind::Dot).is_some()
    }

    pub fn this_type_token(&self) -> Option<SyntaxToken> {
        if !self.is_typed_method() {
            return None;
        }

        let mut saw_fn = false;
        for token in self.signature_tokens() {
            if token.kind().token_kind() == Some(TokenKind::FnKw) {
                saw_fn = true;
                continue;
            }

            if saw_fn
                && matches!(
                    token.kind().token_kind(),
                    Some(TokenKind::Ident | TokenKind::String)
                )
            {
                return Some(token);
            }
        }

        None
    }

    pub fn this_type_name(&self) -> Option<String> {
        let token = self.this_type_token()?;
        let text = token.text().to_string();
        match token.kind().token_kind() {
            Some(TokenKind::Ident) => Some(text),
            Some(TokenKind::String) => text
                .strip_prefix('"')
                .and_then(|text| text.strip_suffix('"'))
                .map(str::to_owned),
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
    pub fn params(&self) -> impl Iterator<Item = SyntaxToken> {
        token_children(&self.syntax)
            .filter(|token| token.kind().token_kind().is_some_and(is_param_token))
    }
}

impl ClosureParamList {
    pub fn params(&self) -> impl Iterator<Item = SyntaxToken> {
        token_children(&self.syntax)
            .filter(|token| token.kind().token_kind().is_some_and(is_param_token))
    }
}

impl AliasClause {
    pub fn alias_token(&self) -> Option<SyntaxToken> {
        find_token(&self.syntax, |kind| {
            matches!(kind, TokenKind::Ident | TokenKind::GlobalKw)
        })
    }
}

impl CatchClause {
    pub fn binding_token(&self) -> Option<SyntaxToken> {
        find_token(&self.syntax, is_binding_token)
    }

    pub fn body(&self) -> Option<BlockExpr> {
        child(&self.syntax)
    }
}
