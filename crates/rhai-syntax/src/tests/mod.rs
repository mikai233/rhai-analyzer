use crate::{Parse, SyntaxKind, SyntaxNode, TokenKind};

pub(crate) mod recovery;
pub(crate) mod rhai_alignment;
pub(crate) mod semicolons;
pub(crate) mod valid;

pub(crate) fn first_stmt_expr(parse: &Parse) -> &SyntaxNode {
    let stmt = parse.root().children()[0]
        .as_node()
        .expect("expected statement node");
    match stmt.kind() {
        SyntaxKind::StmtExpr => stmt.children()[0]
            .as_node()
            .expect("expected expression statement payload"),
        SyntaxKind::StmtLet => stmt.children()[3]
            .as_node()
            .expect("expected let initializer"),
        other => panic!("unexpected statement kind: {other:?}"),
    }
}

pub(crate) fn binary_operator(node: &SyntaxNode) -> TokenKind {
    node.children()[1]
        .as_token()
        .expect("expected operator token")
        .kind()
}

pub(crate) fn binary_rhs(node: &SyntaxNode) -> &SyntaxNode {
    node.children()[2]
        .as_node()
        .expect("expected right-hand side node")
}

pub(crate) fn binary_lhs(node: &SyntaxNode) -> &SyntaxNode {
    node.children()[0]
        .as_node()
        .expect("expected left-hand side node")
}
