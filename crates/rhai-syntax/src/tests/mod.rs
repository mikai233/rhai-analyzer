use crate::{AstNode, Parse, Root, SyntaxKind, SyntaxNode, TokenKind};

pub(crate) mod recovery;
pub(crate) mod rhai_alignment;
pub(crate) mod semicolons;
pub(crate) mod valid;

pub(crate) fn first_stmt_expr(parse: &Parse) -> SyntaxNode {
    let root = Root::cast(parse.root()).expect("expected root");
    let stmt = root
        .item_list()
        .and_then(|items| items.items().next())
        .map(|item| item.syntax().clone())
        .expect("expected statement node");
    match stmt.kind().syntax_kind() {
        Some(crate::SyntaxKind::StmtExpr) => stmt
            .children()
            .next()
            .expect("expected expression statement payload"),
        Some(crate::SyntaxKind::StmtLet) => {
            stmt.children().next().expect("expected let initializer")
        }
        other => panic!("unexpected statement kind: {other:?}"),
    }
}

pub(crate) fn node_kind(node: &SyntaxNode) -> SyntaxKind {
    node.kind()
        .syntax_kind()
        .expect("expected syntax node kind")
}

pub(crate) fn binary_operator(node: &SyntaxNode) -> TokenKind {
    node.children_with_tokens()
        .filter_map(|child| child.into_token())
        .find_map(|token| token.kind().token_kind().filter(|kind| !kind.is_trivia()))
        .expect("expected operator token")
}

pub(crate) fn binary_rhs(node: &SyntaxNode) -> SyntaxNode {
    node.children()
        .nth(1)
        .expect("expected right-hand side node")
}

pub(crate) fn binary_lhs(node: &SyntaxNode) -> SyntaxNode {
    node.children()
        .next()
        .expect("expected left-hand side node")
}
