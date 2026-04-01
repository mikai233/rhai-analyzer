use crate::tests::{first_stmt_expr, node_kind};
use crate::{
    AstNode, CommentKind, RhaiKind, Root, SyntaxKind, TokenKind, TriviaBoundary, parse_text,
};

#[test]
fn parse_exposes_trivia_bearing_rowan_root() {
    let parse = parse_text("let value = 1; // trailing");
    let root = parse.root();
    let kinds: Vec<_> = root
        .descendants_with_tokens()
        .map(|element| element.kind())
        .collect();

    assert_eq!(root.kind(), RhaiKind::Root);
    assert!(kinds.contains(&RhaiKind::StmtLet));
    assert!(kinds.contains(&RhaiKind::LineComment));
}
#[test]
fn parser_skips_shebang_and_all_comment_kinds() {
    let parse = parse_text(
        r#"#!/usr/bin/env rhai
        /// adds a value
        /** block docs */
        let value = 1;
        /* regular block comment */
        // regular line comment
        let other = value + 1;
    "#,
    );

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());
    let root = Root::cast(parse.root()).expect("expected root");
    let item_count = root
        .item_list()
        .map(|items| items.items().count())
        .unwrap_or(0);
    assert_eq!(item_count, 2, "{}", parse.debug_tree());
}
#[test]
fn parse_trivia_store_attaches_trailing_and_leading_comments() {
    let parse = parse_text("let first = 1; // trailing\n/* leading */\nlet second = 2;");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let root = Root::cast(parse.root()).expect("expected root");
    let items = root
        .item_list()
        .map(|items| items.items().collect::<Vec<_>>())
        .expect("expected root item list");
    let first_end = u32::from(items[0].syntax().text_range().end()) as usize;
    let second_start = u32::from(items[1].syntax().text_range().start()) as usize;
    let gap = parse
        .trivia()
        .comment_gap(first_end, second_start, true, true);

    assert_eq!(gap.trailing_comments.len(), 1);
    assert_eq!(gap.trailing_comments[0].kind, CommentKind::Line);
    assert_eq!(gap.trailing_comments[0].text(parse.text()), "// trailing");
    assert_eq!(gap.leading_comments.len(), 1);
    assert_eq!(gap.leading_comments[0].kind, CommentKind::Block);
    assert_eq!(gap.leading_comments[0].text(parse.text()), "/* leading */");
}
#[test]
fn parse_trivia_store_tracks_blank_lines_between_comments_and_next_node() {
    let parse = parse_text("let first = 1;\n\n/* keep */\nlet second = 2;");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let root = Root::cast(parse.root()).expect("expected root");
    let items = root
        .item_list()
        .map(|items| items.items().collect::<Vec<_>>())
        .expect("expected root item list");
    let first_end = u32::from(items[0].syntax().text_range().end()) as usize;
    let second_start = u32::from(items[1].syntax().text_range().start()) as usize;
    let gap = parse
        .trivia()
        .comment_gap(first_end, second_start, true, true);

    assert_eq!(gap.leading_comments.len(), 1);
    assert_eq!(gap.leading_comments[0].text(parse.text()), "/* keep */");
    assert_eq!(gap.leading_comments[0].blank_lines_before, 1);
}
#[test]
fn parse_trivia_store_supports_node_and_token_boundary_queries() {
    let parse = parse_text(
        "let first = value /* before semi */; // trailing\n/* leading */\nlet second = 2;",
    );

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let root = Root::cast(parse.root()).expect("expected root");
    let items = root
        .item_list()
        .map(|items| items.items().collect::<Vec<_>>())
        .expect("expected root item list");
    let first_stmt = items[0].syntax();
    let second_stmt = items[1].syntax();

    assert!(
        parse
            .trivia()
            .boundary_has_comments(&TriviaBoundary::NodeNode(
                first_stmt.clone(),
                second_stmt.clone(),
            ))
    );
    let between_items = parse.trivia().comment_gap_for_boundary(
        &TriviaBoundary::NodeNode(first_stmt.clone(), second_stmt.clone()),
        true,
        true,
    );
    assert_eq!(between_items.trailing_comments.len(), 1);
    assert_eq!(between_items.leading_comments.len(), 1);

    let semicolon = first_stmt
        .children_with_tokens()
        .filter_map(|child| child.into_token())
        .find(|token| token.kind().token_kind() == Some(TokenKind::Semicolon))
        .expect("expected semicolon");
    let value_expr = match &items[0] {
        crate::Item::Stmt(crate::Stmt::Let(let_stmt)) => {
            let_stmt.initializer().expect("expected value")
        }
        _ => panic!("expected let stmt"),
    };

    assert!(
        parse
            .trivia()
            .boundary_has_comments(&TriviaBoundary::NodeToken(
                value_expr.syntax(),
                semicolon.clone(),
            ))
    );
    let before_semicolon = parse.trivia().comment_gap_for_boundary(
        &TriviaBoundary::NodeToken(value_expr.syntax(), semicolon.clone()),
        true,
        true,
    );
    assert_eq!(before_semicolon.trailing_comments.len(), 1);
    assert_eq!(
        before_semicolon.trailing_comments[0].text(parse.text()),
        "/* before semi */"
    );

    assert!(
        parse
            .trivia()
            .boundary_has_comments(&TriviaBoundary::TokenNode(
                semicolon.clone(),
                second_stmt.clone(),
            ))
    );
    let after_semicolon = parse.trivia().comment_gap_for_boundary(
        &TriviaBoundary::TokenNode(semicolon, second_stmt),
        true,
        true,
    );
    assert_eq!(after_semicolon.trailing_comments.len(), 1);
    assert_eq!(after_semicolon.leading_comments.len(), 1);
}
#[test]
fn parse_trivia_store_supports_token_to_token_queries() {
    let parse = parse_text("call( /* keep */ );");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let expr = first_stmt_expr(&parse);
    let args = expr
        .children()
        .find(|node| node_kind(node) == SyntaxKind::ArgList)
        .expect("expected arg list");
    let open_paren = args
        .children_with_tokens()
        .filter_map(|child| child.into_token())
        .find(|token| token.kind().token_kind() == Some(TokenKind::OpenParen))
        .expect("expected open paren");
    let close_paren = args
        .children_with_tokens()
        .filter_map(|child| child.into_token())
        .find(|token| token.kind().token_kind() == Some(TokenKind::CloseParen))
        .expect("expected close paren");

    assert!(
        parse
            .trivia()
            .boundary_has_comments(&TriviaBoundary::TokenToken(
                open_paren.clone(),
                close_paren.clone(),
            ))
    );
    let gap = parse.trivia().comment_gap_for_boundary(
        &TriviaBoundary::TokenToken(open_paren, close_paren),
        false,
        false,
    );
    assert_eq!(gap.dangling_comments.len(), 1);
    assert_eq!(gap.dangling_comments[0].text(parse.text()), "/* keep */");
}
#[test]
fn parse_trivia_store_exposes_owned_slot_trivia() {
    let parse = parse_text("fn Foo /* owner */(value) /* params */ { value }");
    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let root = Root::cast(parse.root()).expect("expected root");
    let item_list = root.item_list().expect("expected root item list");
    let function = item_list
        .items()
        .find_map(|item| match item {
            crate::Item::Fn(function) => Some(function),
            _ => None,
        })
        .expect("expected function item");
    let params = function.params().expect("expected params");
    let last_signature_token = function
        .signature_tokens()
        .last()
        .expect("expected signature token");
    let owner = function.syntax();

    let owned = parse.trivia().owned_trivia(&owner);
    let boundary = TriviaBoundary::TokenNode(last_signature_token, params.syntax());
    let slot = parse
        .trivia()
        .boundary_slot(&owner, &boundary)
        .expect("expected boundary slot");
    let slot_trivia = parse
        .trivia()
        .trivia_for_boundary(&owner, &boundary)
        .expect("expected boundary trivia");

    assert_eq!(
        slot_trivia,
        owned
            .slot(slot)
            .cloned()
            .expect("expected owned slot trivia")
    );
    assert_eq!(slot_trivia.trailing_comments.len(), 1);
    assert_eq!(
        slot_trivia.trailing_comments[0].text(parse.text()),
        "/* owner */"
    );
}
