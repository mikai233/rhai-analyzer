use crate::tests::node_kind;
use crate::{AstNode, Root, SyntaxErrorCode, parse_text};

#[test]
fn recovers_from_missing_commas_in_delimited_lists() {
    let parse = parse_text(
        r#"
        private fn build(x y, z) {
            let values = [1 2, 3];
            let call = run(alpha beta, gamma);
            let map = #{ first: 1 second: 2, third: 3 };
        }
    "#,
    );

    let codes: Vec<_> = parse.errors().iter().map(|error| error.code()).collect();
    assert!(
        codes.contains(&&SyntaxErrorCode::ExpectedCommaBetweenParameters),
        "{codes:?}"
    );
    assert!(
        codes.contains(&&SyntaxErrorCode::ExpectedCommaBetweenArrayItems),
        "{codes:?}"
    );
    assert!(
        codes.contains(&&SyntaxErrorCode::ExpectedCommaBetweenArguments),
        "{codes:?}"
    );
    assert!(
        codes.contains(&&SyntaxErrorCode::ExpectedCommaBetweenObjectFields),
        "{codes:?}"
    );

    let tree = parse.debug_tree();
    assert!(tree.contains("ParamList"), "{tree}");
    assert!(tree.contains("ExprArray"), "{tree}");
    assert!(tree.contains("ExprCall"), "{tree}");
    assert!(tree.contains("ExprObject"), "{tree}");
}
#[test]
fn recovers_when_closure_parameter_list_is_broken() {
    let parse = parse_text("|x y x + y");

    let codes: Vec<_> = parse.errors().iter().map(|error| error.code()).collect();
    assert!(
        codes.contains(&&SyntaxErrorCode::ExpectedCommaBetweenClosureParameters)
            || codes.contains(&&SyntaxErrorCode::ExpectedClosingClosureParameters),
        "{codes:?}"
    );

    let tree = parse.debug_tree();
    assert!(tree.contains("ExprClosure"), "{tree}");
    assert!(tree.contains("ClosureParamList"), "{tree}");
}
#[test]
fn recovers_when_function_parameter_list_runs_into_body() {
    let parse = parse_text(
        r#"
        fn broken(x, { return x; }
        let after = 42;
    "#,
    );

    let codes: Vec<_> = parse.errors().iter().map(|error| error.code()).collect();
    assert!(
        codes.contains(&&SyntaxErrorCode::ExpectedParameterName),
        "{codes:?}"
    );
    assert!(
        codes.contains(&&SyntaxErrorCode::ExpectedClosingParameters),
        "{codes:?}"
    );

    let tree = parse.debug_tree();
    assert!(tree.contains("ItemFn"), "{tree}");
    assert!(tree.contains("StmtReturn"), "{tree}");

    let root = Root::cast(parse.root()).expect("expected root");
    let items = root
        .item_list()
        .map(|items| items.items().collect::<Vec<_>>())
        .unwrap_or_default();
    assert_eq!(items.len(), 2, "{}", tree);
    let second_stmt = items[1].syntax();
    assert_eq!(node_kind(&second_stmt), crate::SyntaxKind::StmtLet);
}
#[test]
fn recovers_when_closure_parameter_list_runs_into_block_body() {
    let parse = parse_text("let f = |x, { x + 1 }; let after = 1;");

    let codes: Vec<_> = parse.errors().iter().map(|error| error.code()).collect();
    assert!(
        codes.contains(&&SyntaxErrorCode::ExpectedClosureParameter),
        "{codes:?}"
    );
    assert!(
        codes.contains(&&SyntaxErrorCode::ExpectedClosingClosureParameters),
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
