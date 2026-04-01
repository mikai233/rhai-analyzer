use crate::tests::node_kind;
use crate::{AstNode, Root, SyntaxErrorCode, parse_text};

#[test]
fn recovers_from_missing_expression() {
    let parse = parse_text("let value = ;");

    assert_eq!(parse.errors().len(), 1);
    assert_eq!(
        parse.errors()[0].code(),
        &SyntaxErrorCode::ExpectedExpression
    );

    let root = Root::cast(parse.root()).expect("expected root");
    let stmt = root
        .item_list()
        .and_then(|items| items.items().next())
        .map(|item| item.syntax())
        .expect("expected statement node");
    assert_eq!(node_kind(&stmt), crate::SyntaxKind::StmtLet);

    let has_error_node = stmt
        .children()
        .any(|node| node_kind(&node) == crate::SyntaxKind::Error);
    assert!(has_error_node, "{}", parse.debug_tree());
}
#[test]
fn recovers_from_missing_object_field_value() {
    let parse = parse_text("#{ answer: }");

    assert_eq!(parse.errors().len(), 1, "{}", parse.debug_tree());
    assert_eq!(
        parse.errors()[0].code(),
        &SyntaxErrorCode::ExpectedPropertyValue
    );

    let tree = parse.debug_tree();
    assert!(tree.contains("ExprObject"), "{tree}");
    assert!(tree.contains("ObjectField"), "{tree}");
    assert!(tree.contains("Error"), "{tree}");
}
#[test]
fn recovers_across_statement_boundary_after_broken_call() {
    let parse = parse_text(
        r#"
        let first = run(1 2;
        let second = 42;
    "#,
    );

    let codes: Vec<_> = parse.errors().iter().map(|error| error.code()).collect();
    assert!(
        codes.contains(&&SyntaxErrorCode::ExpectedCommaBetweenArguments),
        "{codes:?}"
    );
    assert!(
        codes.contains(&&SyntaxErrorCode::ExpectedClosingArgumentList),
        "{codes:?}"
    );

    let root = Root::cast(parse.root()).expect("expected root");
    let items = root
        .item_list()
        .map(|items| items.items().collect::<Vec<_>>())
        .unwrap_or_default();
    assert_eq!(items.len(), 2, "{}", parse.debug_tree());

    let second_stmt = items[1].syntax();
    assert_eq!(node_kind(&second_stmt), crate::SyntaxKind::StmtLet);
}
#[test]
fn recovers_across_statement_boundary_after_missing_binary_rhs() {
    let parse = parse_text(
        r#"
        let first = 1 + ;
        let second = 42;
    "#,
    );

    let codes: Vec<_> = parse.errors().iter().map(|error| error.code()).collect();
    assert!(
        codes.contains(&&SyntaxErrorCode::ExpectedExpressionAfterOperator),
        "{codes:?}"
    );

    let root = Root::cast(parse.root()).expect("expected root");
    let items = root
        .item_list()
        .map(|items| items.items().collect::<Vec<_>>())
        .unwrap_or_default();
    assert_eq!(items.len(), 2, "{}", parse.debug_tree());
    let second_stmt = items[1].syntax();
    assert_eq!(node_kind(&second_stmt), crate::SyntaxKind::StmtLet);
}
