use crate::tests::{binary_lhs, binary_operator, binary_rhs, first_stmt_expr, node_kind};
use crate::{AstNode, Root, SyntaxKind, TokenKind, parse_text};

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
