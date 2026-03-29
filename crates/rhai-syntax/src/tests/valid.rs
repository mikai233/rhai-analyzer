use crate::tests::{binary_lhs, binary_operator, binary_rhs, first_stmt_expr};
use crate::{SyntaxKind, TokenKind, lex_text, parse_text};

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
    assert!(kinds.contains(&TokenKind::BacktickString));
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
            TokenKind::BacktickString,
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
    assert_eq!(parse.root().children().len(), 2, "{}", parse.debug_tree());
}

#[test]
fn parses_let_statement_with_call_and_binary_expr() {
    let parse = parse_text("let answer = add(1, 2) + 3;");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let root = parse.root();
    assert_eq!(root.kind(), SyntaxKind::Root);
    assert_eq!(root.children().len(), 1);

    let stmt = root.children()[0]
        .as_node()
        .expect("expected statement node");
    assert_eq!(stmt.kind(), SyntaxKind::StmtLet);

    let tree = parse.debug_tree();
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
    assert_eq!(expr.kind(), SyntaxKind::ExprAssign);

    let rhs = binary_rhs(expr);
    assert_eq!(rhs.kind(), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(rhs), TokenKind::StarStar);

    let unary_operand = binary_lhs(rhs);
    assert_eq!(unary_operand.kind(), SyntaxKind::ExprUnary);
}

#[test]
fn assignment_is_right_associative() {
    let parse = parse_text("a = b = c;");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let expr = first_stmt_expr(&parse);
    assert_eq!(expr.kind(), SyntaxKind::ExprAssign);

    let rhs = binary_rhs(expr);
    assert_eq!(rhs.kind(), SyntaxKind::ExprAssign);
}

#[test]
fn logical_precedence_groups_tighter_than_or() {
    let parse = parse_text("a == b || c && d in xs;");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let expr = first_stmt_expr(&parse);
    assert_eq!(expr.kind(), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(expr), TokenKind::PipePipe);

    let rhs = binary_rhs(expr);
    assert_eq!(rhs.kind(), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(rhs), TokenKind::AmpAmp);

    let nested_rhs = binary_rhs(rhs);
    assert_eq!(nested_rhs.kind(), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(nested_rhs), TokenKind::InKw);
}

#[test]
fn unary_binds_tighter_than_exponent_in_rhai() {
    let parse = parse_text("-2 ** 2;");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let expr = first_stmt_expr(&parse);
    assert_eq!(expr.kind(), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(expr), TokenKind::StarStar);
    assert_eq!(binary_lhs(expr).kind(), SyntaxKind::ExprUnary);
}

#[test]
fn shift_binds_tighter_than_exponent_and_addition() {
    let parse = parse_text("1 + 2 << 3 ** 4;");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let expr = first_stmt_expr(&parse);
    assert_eq!(expr.kind(), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(expr), TokenKind::Plus);

    let rhs = binary_rhs(expr);
    assert_eq!(rhs.kind(), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(rhs), TokenKind::StarStar);

    let exp_lhs = binary_lhs(rhs);
    assert_eq!(exp_lhs.kind(), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(exp_lhs), TokenKind::Shl);
}

#[test]
fn bitwise_and_logical_same_precedence_groups_are_left_associative() {
    let parse = parse_text("a | b ^ c || d;");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let expr = first_stmt_expr(&parse);
    assert_eq!(expr.kind(), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(expr), TokenKind::PipePipe);

    let lhs = binary_lhs(expr);
    assert_eq!(lhs.kind(), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(lhs), TokenKind::Caret);

    let nested_lhs = binary_lhs(lhs);
    assert_eq!(nested_lhs.kind(), SyntaxKind::ExprBinary);
    assert_eq!(binary_operator(nested_lhs), TokenKind::Pipe);
}

#[test]
fn parses_if_else_chain_in_expression_position() {
    let parse = parse_text("let value = if flag { 1 } else if other { 2 } else { 3 };");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let expr = first_stmt_expr(&parse);
    assert_eq!(expr.kind(), SyntaxKind::ExprIf);

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
    assert_eq!(expr.kind(), SyntaxKind::ExprSwitch);

    let tree = parse.debug_tree();
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
      OpenParen "("
      ArgList
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
    assert!(tree.contains("StringSegment"), "{tree}");
    assert!(tree.contains("StringInterpolation"), "{tree}");
    assert!(tree.contains("InterpolationBody"), "{tree}");
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
  StmtLet
    LetKw "let"
    Ident "msg"
    Eq "="
    ExprInterpolatedString
      Backtick "`"
      StringSegment
        StringText "value="
      StringInterpolation
        InterpolationStart "${"
        InterpolationBody
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
