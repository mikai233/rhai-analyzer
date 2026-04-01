use crate::tests::{binary_lhs, binary_operator, binary_rhs, first_stmt_expr, node_kind};
use crate::{
    AstNode, CommentKind, RhaiKind, Root, SyntaxKind, TokenKind, TriviaBoundary, lex_text,
    parse_text,
};

#[test]
fn lexes_basic_tokens() {
    let lexed = lex_text("let answer = add(1, 2)");
    let kinds: Vec<_> = lexed
        .tokens()
        .iter()
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.kind())
        .collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::LetKw,
            TokenKind::Ident,
            TokenKind::Eq,
            TokenKind::Ident,
            TokenKind::OpenParen,
            TokenKind::Int,
            TokenKind::Comma,
            TokenKind::Int,
            TokenKind::CloseParen,
        ]
    );
    assert!(lexed.errors().is_empty());
}

#[test]
fn syntax_kind_round_trips_through_rowan_kind() {
    let kinds = [
        SyntaxKind::Root,
        SyntaxKind::StmtLet,
        SyntaxKind::ExprBinary,
        SyntaxKind::ArgList,
        SyntaxKind::CatchClause,
        SyntaxKind::Error,
    ];

    for kind in kinds {
        let raw = kind.to_rowan();
        assert_eq!(SyntaxKind::from_rowan(raw), Some(kind));
    }

    assert_eq!(SyntaxKind::from_rowan(rowan::SyntaxKind(u16::MAX)), None);
}

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
fn interpolation_body_parser_errors_use_absolute_ranges() {
    let source = "let msg = `value = ${1 + }`;";
    let parse = parse_text(source);
    let error = parse
        .errors()
        .iter()
        .find(|error| error.message() == "expected expression after operator")
        .expect("expected interpolation body parser error");

    let start = u32::from(error.range().start()) as usize;
    assert!(start > source.find("${").expect("expected interpolation start"));
}

#[test]
fn lexes_extended_rhai_token_set() {
    let source = r#"
        const answer = #{ value: 42, text: `hello ${name}` };
        if answer.value >= 40 && "value" in answer {
            answer.value += 1;
            answer?.items?[0] ??= 0;
        }
        /* nested /* block */ comment */
    "#;
    let lexed = lex_text(source);
    let kinds: Vec<_> = lexed
        .tokens()
        .iter()
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.kind())
        .collect();

    assert!(kinds.contains(&TokenKind::ConstKw));
    assert!(kinds.contains(&TokenKind::HashBraceOpen));
    assert!(kinds.contains(&TokenKind::Colon));
    assert!(kinds.contains(&TokenKind::Backtick));
    assert!(kinds.contains(&TokenKind::StringText));
    assert!(kinds.contains(&TokenKind::InterpolationStart));
    assert!(kinds.contains(&TokenKind::IfKw));
    assert!(kinds.contains(&TokenKind::GtEq));
    assert!(kinds.contains(&TokenKind::AmpAmp));
    assert!(kinds.contains(&TokenKind::InKw));
    assert!(kinds.contains(&TokenKind::PlusEq));
    assert!(kinds.contains(&TokenKind::QuestionDot));
    assert!(kinds.contains(&TokenKind::QuestionOpenBracket));
    assert!(kinds.contains(&TokenKind::QuestionQuestionEq));
    assert!(lexed.errors().is_empty(), "{:?}", lexed.errors());
}

#[test]
fn lexes_doc_comments_and_shebang() {
    let source = "#!/usr/bin/env rhai\n/// docs\n//! module docs\n/** block docs */\n/*! inner docs */\nlet value = 1;";
    let lexed = lex_text(source);
    let kinds: Vec<_> = lexed.tokens().iter().map(|token| token.kind()).collect();

    assert_eq!(kinds[0], TokenKind::Shebang);
    assert!(kinds.contains(&TokenKind::DocLineComment));
    assert!(kinds.contains(&TokenKind::DocBlockComment));
    assert!(lexed.errors().is_empty(), "{:?}", lexed.errors());
}

#[test]
fn lexes_raw_strings_number_edges_and_remaining_operators() {
    let source = r###"
        let raw = #"alpha"#;
        let deeper = ##"beta"##;
        let plain = 42;
        let float = 3.14;
        let exp = 6.02e23;
        let fallback = 10e + 2;
        value <<= 1;
        value >>= 2;
        value &= 3;
        value |= 4;
        value ^= 5;
        value **= 6;
        if left == right != maybe { result = 1..=3; other = 4..9; }
        @
    "###;
    let lexed = lex_text(source);
    let kinds: Vec<_> = lexed
        .tokens()
        .iter()
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.kind())
        .collect();

    assert!(kinds.contains(&TokenKind::RawString));
    assert!(kinds.contains(&TokenKind::Float));
    assert!(kinds.contains(&TokenKind::ShlEq));
    assert!(kinds.contains(&TokenKind::ShrEq));
    assert!(kinds.contains(&TokenKind::AmpEq));
    assert!(kinds.contains(&TokenKind::PipeEq));
    assert!(kinds.contains(&TokenKind::CaretEq));
    assert!(kinds.contains(&TokenKind::StarStarEq));
    assert!(kinds.contains(&TokenKind::EqEq));
    assert!(kinds.contains(&TokenKind::BangEq));
    assert!(kinds.contains(&TokenKind::RangeEq));
    assert!(kinds.contains(&TokenKind::Range));
    assert!(kinds.contains(&TokenKind::Unknown));

    assert!(
        kinds.windows(4).any(|window| {
            window
                == [
                    TokenKind::Int,
                    TokenKind::Ident,
                    TokenKind::Plus,
                    TokenKind::Int,
                ]
        }),
        "{kinds:?}"
    );
    assert!(lexed.errors().is_empty(), "{:?}", lexed.errors());
}

#[test]
fn lexes_interpolation_with_nested_lexical_forms() {
    let source =
        r###"let value = `head ${ foo(1, #"raw"#, `inner ${bar}`) /* note */ + "tail" } tail`;"###;
    let lexed = lex_text(source);
    let kinds: Vec<_> = lexed
        .tokens()
        .iter()
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.kind())
        .collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::LetKw,
            TokenKind::Ident,
            TokenKind::Eq,
            TokenKind::Backtick,
            TokenKind::StringText,
            TokenKind::InterpolationStart,
            TokenKind::Ident,
            TokenKind::OpenParen,
            TokenKind::Int,
            TokenKind::Comma,
            TokenKind::RawString,
            TokenKind::Comma,
            TokenKind::Backtick,
            TokenKind::StringText,
            TokenKind::InterpolationStart,
            TokenKind::Ident,
            TokenKind::CloseBrace,
            TokenKind::Backtick,
            TokenKind::CloseParen,
            TokenKind::Plus,
            TokenKind::String,
            TokenKind::CloseBrace,
            TokenKind::StringText,
            TokenKind::Backtick,
            TokenKind::Semicolon,
        ]
    );
    assert!(lexed.errors().is_empty(), "{:?}", lexed.errors());
}

#[test]
fn reports_unterminated_string_like_literals() {
    let string_lexed = lex_text("\"unterminated");
    assert_eq!(string_lexed.errors().len(), 1);
    assert_eq!(
        string_lexed.errors()[0].message(),
        "unterminated string literal"
    );

    let char_lexed = lex_text("'x");
    assert_eq!(char_lexed.errors().len(), 1);
    assert_eq!(
        char_lexed.errors()[0].message(),
        "unterminated character literal"
    );

    let raw_lexed = lex_text("#\"raw");
    assert_eq!(raw_lexed.errors().len(), 1);
    assert_eq!(
        raw_lexed.errors()[0].message(),
        "unterminated raw string literal"
    );

    let backtick_lexed = lex_text("`value");
    assert_eq!(backtick_lexed.errors().len(), 1);
    assert_eq!(
        backtick_lexed.errors()[0].message(),
        "unterminated back-tick string literal"
    );
}

#[test]
fn reports_unterminated_interpolation_and_block_comments() {
    let interpolation_lexed = lex_text("`value = ${foo(1)`");
    let interpolation_messages: Vec<_> = interpolation_lexed
        .errors()
        .iter()
        .map(|error| error.message())
        .collect();
    assert!(
        interpolation_messages.contains(&"unterminated string interpolation"),
        "{interpolation_messages:?}"
    );
    assert!(
        interpolation_messages.contains(&"unterminated back-tick string literal"),
        "{interpolation_messages:?}"
    );
    assert!(
        interpolation_messages.len() >= 2,
        "{interpolation_messages:?}"
    );

    let block_lexed = lex_text("/* outer /* inner */");
    assert_eq!(block_lexed.errors().len(), 1);
    assert_eq!(
        block_lexed.errors()[0].message(),
        "unterminated block comment"
    );
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

#[test]
fn parses_let_statement_with_call_and_binary_expr() {
    let parse = parse_text("let answer = add(1, 2) + 3;");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let root = Root::cast(parse.root()).expect("expected root");
    assert_eq!(node_kind(&root.syntax()), SyntaxKind::Root);
    let item_list = root.item_list().expect("expected root item list");
    let items = item_list.items().collect::<Vec<_>>();
    assert_eq!(items.len(), 1);

    let stmt = items[0].syntax();
    assert_eq!(node_kind(&stmt), SyntaxKind::StmtLet);

    let tree = parse.debug_tree();
    assert!(tree.contains("RootItemList"), "{tree}");
    assert!(tree.contains("ExprBinary"), "{tree}");
    assert!(tree.contains("ExprCall"), "{tree}");
    assert!(tree.contains("ArgList"), "{tree}");
}

#[test]
fn parses_array_object_and_access_chains() {
    let parse = parse_text(r#"#{ data: [1, 2, 3], nested: #{ item: 42 } }.nested?.item + arr?[0]"#);

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let tree = parse.debug_tree();
    assert!(tree.contains("ExprObject"), "{tree}");
    assert!(tree.contains("ObjectFieldList"), "{tree}");
    assert!(tree.contains("ObjectField"), "{tree}");
    assert!(tree.contains("ExprArray"), "{tree}");
    assert!(tree.contains("ExprField"), "{tree}");
    assert!(tree.contains("ExprIndex"), "{tree}");
}

#[test]
fn parses_unary_and_assignment_expressions() {
    let parse = parse_text("target.value ??= -2 ** 3;");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let expr = first_stmt_expr(&parse);
    assert_eq!(node_kind(&expr), SyntaxKind::ExprAssign);

    let rhs = binary_rhs(&expr);
    assert_eq!(node_kind(&rhs), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(&rhs), TokenKind::StarStar);

    let unary_operand = binary_lhs(&rhs);
    assert_eq!(node_kind(&unary_operand), SyntaxKind::ExprUnary);
}

#[test]
fn assignment_is_right_associative() {
    let parse = parse_text("a = b = c;");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let expr = first_stmt_expr(&parse);
    assert_eq!(node_kind(&expr), SyntaxKind::ExprAssign);

    let rhs = binary_rhs(&expr);
    assert_eq!(node_kind(&rhs), SyntaxKind::ExprAssign);
}

#[test]
fn logical_precedence_groups_tighter_than_or() {
    let parse = parse_text("a == b || c && d in xs;");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let expr = first_stmt_expr(&parse);
    assert_eq!(node_kind(&expr), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(&expr), TokenKind::PipePipe);

    let rhs = binary_rhs(&expr);
    assert_eq!(node_kind(&rhs), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(&rhs), TokenKind::AmpAmp);

    let nested_rhs = binary_rhs(&rhs);
    assert_eq!(node_kind(&nested_rhs), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(&nested_rhs), TokenKind::InKw);
}

#[test]
fn unary_binds_tighter_than_exponent_in_rhai() {
    let parse = parse_text("-2 ** 2;");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let expr = first_stmt_expr(&parse);
    assert_eq!(node_kind(&expr), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(&expr), TokenKind::StarStar);
    assert_eq!(node_kind(&binary_lhs(&expr)), SyntaxKind::ExprUnary);
}

#[test]
fn shift_binds_tighter_than_exponent_and_addition() {
    let parse = parse_text("1 + 2 << 3 ** 4;");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let expr = first_stmt_expr(&parse);
    assert_eq!(node_kind(&expr), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(&expr), TokenKind::Plus);

    let rhs = binary_rhs(&expr);
    assert_eq!(node_kind(&rhs), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(&rhs), TokenKind::StarStar);

    let exp_lhs = binary_lhs(&rhs);
    assert_eq!(node_kind(&exp_lhs), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(&exp_lhs), TokenKind::Shl);
}

#[test]
fn bitwise_and_logical_same_precedence_groups_are_left_associative() {
    let parse = parse_text("a | b ^ c || d;");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let expr = first_stmt_expr(&parse);
    assert_eq!(node_kind(&expr), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(&expr), TokenKind::PipePipe);

    let lhs = binary_lhs(&expr);
    assert_eq!(node_kind(&lhs), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(&lhs), TokenKind::Caret);

    let nested_lhs = binary_lhs(&lhs);
    assert_eq!(node_kind(&nested_lhs), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(&nested_lhs), TokenKind::Pipe);
}

#[test]
fn parses_if_else_chain_in_expression_position() {
    let parse = parse_text("let value = if flag { 1 } else if other { 2 } else { 3 };");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let expr = first_stmt_expr(&parse);
    assert_eq!(node_kind(&expr), SyntaxKind::ExprIf);

    let tree = parse.debug_tree();
    assert!(tree.contains("ElseBranch"), "{tree}");
    assert!(tree.matches("ExprIf").count() >= 2, "{tree}");
}

#[test]
fn parses_looping_constructs() {
    let parse = parse_text(
        "for (item, index) in items { while index < 10 { continue; } } loop { break 1; }",
    );

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let tree = parse.debug_tree();
    assert!(tree.contains("ExprFor"), "{tree}");
    assert!(tree.contains("ForBindings"), "{tree}");
    assert!(tree.contains("ExprWhile"), "{tree}");
    assert!(tree.contains("ExprLoop"), "{tree}");
    assert!(tree.contains("StmtContinue"), "{tree}");
    assert!(tree.contains("StmtBreak"), "{tree}");
}

#[test]
fn parses_try_catch_and_value_statements() {
    let parse = parse_text("try { throw err; } catch (error) { return error; }");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let tree = parse.debug_tree();
    assert!(tree.contains("StmtTry"), "{tree}");
    assert!(tree.contains("CatchClause"), "{tree}");
    assert!(tree.contains("StmtThrow"), "{tree}");
    assert!(tree.contains("StmtReturn"), "{tree}");
}

#[test]
fn parses_switch_expression_with_patterns() {
    let parse = parse_text(
        "let kind = switch value { 0 => `zero`, 1 | 2 => `small`, _ => { return `many`; } };",
    );

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let expr = first_stmt_expr(&parse);
    assert_eq!(node_kind(&expr), SyntaxKind::ExprSwitch);

    let tree = parse.debug_tree();
    assert!(tree.contains("BlockItemList"), "{tree}");
    assert!(tree.contains("SwitchArmList"), "{tree}");
    assert!(tree.contains("SwitchArm"), "{tree}");
    assert!(tree.contains("SwitchPatternList"), "{tree}");
    assert!(tree.contains("Block"), "{tree}");
}

#[test]
fn parses_do_while_and_do_until() {
    let parse = parse_text("do { x += 1; } while x < 10; do { x -= 1; } until x == 0;");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let tree = parse.debug_tree();
    assert!(tree.matches("ExprDo").count() >= 2, "{tree}");
    assert!(tree.contains("DoCondition"), "{tree}");
}

#[test]
fn compact_snapshot_for_valid_program() {
    let parse = parse_text(
        r#"private fn add(x, y,) { return x + y; }
const ANSWER = add(20, 22);
let kind = switch ANSWER { 42 => `yes`, _ => `no` };"#,
    );

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let expected = r#"Root
  RootItemList
    ItemFn
      PrivateKw "private"
      FnKw "fn"
      Ident "add"
      ParamList
        OpenParen "("
        Ident "x"
        Comma ","
        Ident "y"
        Comma ","
        CloseParen ")"
      Block
        OpenBrace "{"
        BlockItemList
          StmtReturn
            ReturnKw "return"
            ExprBinary
              ExprName
                Ident "x"
              Plus "+"
              ExprName
                Ident "y"
            Semicolon ";"
        CloseBrace "}"
    StmtConst
      ConstKw "const"
      Ident "ANSWER"
      Eq "="
      ExprCall
        ExprName
          Ident "add"
        ArgList
          OpenParen "("
          ExprLiteral
            Int "20"
          Comma ","
          ExprLiteral
            Int "22"
          CloseParen ")"
      Semicolon ";"
    StmtLet
      LetKw "let"
      Ident "kind"
      Eq "="
      ExprSwitch
        SwitchKw "switch"
        ExprName
          Ident "ANSWER"
        OpenBrace "{"
        SwitchArmList
          SwitchArm
            SwitchPatternList
              ExprLiteral
                Int "42"
            FatArrow "=>"
            ExprLiteral
              BacktickString "`yes`"
          Comma ","
          SwitchArm
            SwitchPatternList
              Underscore "_"
            FatArrow "=>"
            ExprLiteral
              BacktickString "`no`"
        CloseBrace "}"
      Semicolon ";"
"#;

    assert_eq!(parse.debug_tree_compact(), expected);
}

#[test]
fn parses_closures_and_function_pointer_calls() {
    let parse = parse_text(
        r#"
        let add = |x, y| x + y;
        let thunk = || { return Fn("calc").curry(40).call(2); };
        list.push(|value| value.type_of());
    "#,
    );

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let tree = parse.debug_tree();
    assert!(tree.contains("ExprClosure"), "{tree}");
    assert!(tree.contains("ClosureParamList"), "{tree}");
    assert!(tree.contains("FnPtrKw"), "{tree}");
    assert!(tree.contains("CallKw"), "{tree}");
    assert!(tree.contains("CurryKw"), "{tree}");
}

#[test]
fn parses_interpolated_string_structure() {
    let parse = parse_text(r#"let message = `hello ${name}, value = ${1 + 2}`;"#);

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let tree = parse.debug_tree();
    assert!(tree.contains("ExprInterpolatedString"), "{tree}");
    assert!(tree.contains("StringPartList"), "{tree}");
    assert!(tree.contains("StringSegment"), "{tree}");
    assert!(tree.contains("StringInterpolation"), "{tree}");
    assert!(tree.contains("InterpolationBody"), "{tree}");
    assert!(tree.contains("InterpolationItemList"), "{tree}");
}

#[test]
fn parses_nested_backtick_strings_inside_interpolation() {
    let parse = parse_text(r#"let message = `outer ${`inner ${name}`}`;"#);

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let tree = parse.debug_tree();
    assert!(
        tree.matches("ExprInterpolatedString").count() >= 2,
        "{tree}"
    );
    assert!(tree.matches("StringInterpolation").count() >= 2, "{tree}");
}

#[test]
fn compact_snapshot_for_interpolated_string() {
    let parse = parse_text(r#"let msg = `value=${x + 1}`;"#);

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let expected = r#"Root
  RootItemList
    StmtLet
      LetKw "let"
      Ident "msg"
      Eq "="
      ExprInterpolatedString
        Backtick "`"
        StringPartList
          StringSegment
            StringText "value="
          StringInterpolation
            InterpolationStart "${"
            InterpolationBody
              InterpolationItemList
                StmtExpr
                  ExprBinary
                    ExprName
                      Ident "x"
                    Plus "+"
                    ExprLiteral
                      Int "1"
            CloseBrace "}"
        Backtick "`"
      Semicolon ";"
"#;

    assert_eq!(parse.debug_tree_compact(), expected);
}
