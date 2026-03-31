use crate::{AstNode, Item, Parse, Root, SyntaxKind, SyntaxNode, TokenKind};

pub(crate) mod recovery;
pub(crate) mod rhai_alignment;
pub(crate) mod semicolons;
pub(crate) mod valid;

pub(crate) fn first_stmt_expr(parse: &Parse) -> &SyntaxNode {
    let root = Root::cast(parse.root()).expect("expected root");
    let stmt = root
        .item_list()
        .and_then(|items| items.items().next())
        .map(Item::syntax)
        .expect("expected statement node");
    match stmt.kind() {
        SyntaxKind::StmtExpr => stmt
            .children()
            .iter()
            .filter_map(|child| child.as_node())
            .next()
            .expect("expected expression statement payload"),
        SyntaxKind::StmtLet => stmt
            .children()
            .iter()
            .filter_map(|child| child.as_node())
            .next()
            .expect("expected let initializer"),
        other => panic!("unexpected statement kind: {other:?}"),
    }
}

pub(crate) fn binary_operator(node: &SyntaxNode) -> TokenKind {
    node.children()
        .iter()
        .filter_map(|child| child.as_token())
        .find(|token| !token.kind().is_trivia())
        .expect("expected operator token")
        .kind()
}

pub(crate) fn binary_rhs(node: &SyntaxNode) -> &SyntaxNode {
    node.children()
        .iter()
        .filter_map(|child| child.as_node())
        .nth(1)
        .expect("expected right-hand side node")
}

pub(crate) fn binary_lhs(node: &SyntaxNode) -> &SyntaxNode {
    node.children()
        .iter()
        .filter_map(|child| child.as_node())
        .next()
        .expect("expected left-hand side node")
}
