use crate::{AstNode, Item, Root, SyntaxElement, SyntaxKind, parse_text};

#[test]
fn recovers_from_missing_expression() {
    let parse = parse_text("let value = ;");

    assert_eq!(parse.errors().len(), 1);
    assert_eq!(parse.errors()[0].message(), "expected expression");

    let root = Root::cast(parse.root()).expect("expected root");
    let stmt = root
        .item_list()
        .and_then(|items| items.items().next())
        .map(Item::syntax)
        .expect("expected statement node");
    assert_eq!(stmt.kind(), SyntaxKind::StmtLet);

    let has_error_node = stmt.children().iter().any(|element| {
        matches!(
            element,
            SyntaxElement::Node(node) if node.kind() == SyntaxKind::Error
        )
    });
    assert!(has_error_node, "{}", parse.debug_tree());
}

#[test]
fn recovers_from_missing_object_field_value() {
    let parse = parse_text("#{ answer: }");

    assert_eq!(parse.errors().len(), 1, "{}", parse.debug_tree());
    assert_eq!(parse.errors()[0].message(), "expected property value");

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

    let messages: Vec<_> = parse.errors().iter().map(|error| error.message()).collect();
    assert!(
        messages.contains(&"expected `,` between arguments"),
        "{messages:?}"
    );
    assert!(
        messages.contains(&"expected `)` to close argument list"),
        "{messages:?}"
    );

    let root = Root::cast(parse.root()).expect("expected root");
    let items = root
        .item_list()
        .map(|items| items.items().collect::<Vec<_>>())
        .unwrap_or_default();
    assert_eq!(items.len(), 2, "{}", parse.debug_tree());

    let second_stmt = items[1].syntax();
    assert_eq!(second_stmt.kind(), SyntaxKind::StmtLet);
}

#[test]
fn recovers_across_statement_boundary_after_missing_binary_rhs() {
    let parse = parse_text(
        r#"
        let first = 1 + ;
        let second = 42;
    "#,
    );

    let messages: Vec<_> = parse.errors().iter().map(|error| error.message()).collect();
    assert!(
        messages.contains(&"expected expression after operator"),
        "{messages:?}"
    );

    let root = Root::cast(parse.root()).expect("expected root");
    let items = root
        .item_list()
        .map(|items| items.items().collect::<Vec<_>>())
        .unwrap_or_default();
    assert_eq!(items.len(), 2, "{}", parse.debug_tree());
    let second_stmt = items[1].syntax();
    assert_eq!(second_stmt.kind(), SyntaxKind::StmtLet);
}

#[test]
fn recovers_when_for_is_missing_in_keyword() {
    let parse = parse_text("for value values { break; }");

    assert_eq!(parse.errors().len(), 1, "{}", parse.debug_tree());
    assert_eq!(
        parse.errors()[0].message(),
        "expected `in` in `for` expression"
    );

    let tree = parse.debug_tree();
    assert!(tree.contains("ExprFor"), "{tree}");
    assert!(tree.contains("ForBindings"), "{tree}");
}

#[test]
fn recovers_when_switch_arm_is_missing_arrow() {
    let parse = parse_text("switch value { 1 `one`, _ => `other` }");

    assert_eq!(parse.errors().len(), 1, "{}", parse.debug_tree());
    assert_eq!(parse.errors()[0].message(), "expected `=>` in `switch` arm");

    let tree = parse.debug_tree();
    assert!(tree.contains("ExprSwitch"), "{tree}");
    assert!(tree.contains("SwitchArmList"), "{tree}");
    assert!(tree.contains("SwitchArm"), "{tree}");
}

#[test]
fn recovers_when_const_is_missing_value() {
    let parse = parse_text("const ANSWER = ;");

    assert_eq!(parse.errors().len(), 1, "{}", parse.debug_tree());
    assert_eq!(parse.errors()[0].message(), "expected constant value");

    let tree = parse.debug_tree();
    assert!(tree.contains("StmtConst"), "{tree}");
    assert!(tree.contains("Error"), "{tree}");
}

#[test]
fn recovers_when_alias_is_missing_after_as() {
    let parse = parse_text(r#"import "crypto" as ;"#);

    assert_eq!(parse.errors().len(), 1, "{}", parse.debug_tree());
    assert_eq!(parse.errors()[0].message(), "expected alias after `as`");

    let tree = parse.debug_tree();
    assert!(tree.contains("StmtImport"), "{tree}");
    assert!(tree.contains("AliasClause"), "{tree}");
    assert!(tree.contains("Error"), "{tree}");
}

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

    let messages: Vec<_> = parse.errors().iter().map(|error| error.message()).collect();
    assert!(
        messages.contains(&"expected `,` between parameters"),
        "{messages:?}"
    );
    assert!(
        messages.contains(&"expected `,` between array items"),
        "{messages:?}"
    );
    assert!(
        messages.contains(&"expected `,` between arguments"),
        "{messages:?}"
    );
    assert!(
        messages.contains(&"expected `,` between object fields"),
        "{messages:?}"
    );

    let tree = parse.debug_tree();
    assert!(tree.contains("ParamList"), "{tree}");
    assert!(tree.contains("ExprArray"), "{tree}");
    assert!(tree.contains("ExprCall"), "{tree}");
    assert!(tree.contains("ExprObject"), "{tree}");
}

#[test]
fn compact_snapshot_for_broken_program() {
    let parse = parse_text(
        r#"fn broken(x y {
let values = [1 2];
import "mod" as ;
}"#,
    );

    let expected = r#"Root
  RootItemList
    ItemFn
      FnKw "fn"
      Ident "broken"
      ParamList
        OpenParen "("
        Ident "x"
        Error
        Ident "y"
        Error
      Block
        OpenBrace "{"
        BlockItemList
          StmtLet
            LetKw "let"
            Ident "values"
            Eq "="
            ExprArray
              ArrayItemList
                OpenBracket "["
                ExprLiteral
                  Int "1"
                Error
                ExprLiteral
                  Int "2"
                CloseBracket "]"
            Semicolon ";"
          StmtImport
            ImportKw "import"
            ExprLiteral
              String "\"mod\""
            AliasClause
              AsKw "as"
              Error
            Semicolon ";"
        CloseBrace "}"
"#;

    assert_eq!(parse.debug_tree_compact(), expected);
}

#[test]
fn recovers_when_closure_parameter_list_is_broken() {
    let parse = parse_text("|x y x + y");

    let messages: Vec<_> = parse.errors().iter().map(|error| error.message()).collect();
    assert!(
        messages.contains(&"expected `,` between closure parameters")
            || messages.contains(&"expected closing `|` for closure parameters"),
        "{messages:?}"
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

    let messages: Vec<_> = parse.errors().iter().map(|error| error.message()).collect();
    assert!(
        messages.contains(&"expected parameter name"),
        "{messages:?}"
    );
    assert!(
        messages.contains(&"expected `)` after parameters"),
        "{messages:?}"
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
    assert_eq!(second_stmt.kind(), SyntaxKind::StmtLet);
}

#[test]
fn recovers_when_closure_parameter_list_runs_into_block_body() {
    let parse = parse_text("let f = |x, { x + 1 }; let after = 1;");

    let messages: Vec<_> = parse.errors().iter().map(|error| error.message()).collect();
    assert!(
        messages.contains(&"expected closure parameter"),
        "{messages:?}"
    );
    assert!(
        messages.contains(&"expected closing `|` for closure parameters"),
        "{messages:?}"
    );

    let root = Root::cast(parse.root()).expect("expected root");
    let items = root
        .item_list()
        .map(|items| items.items().collect::<Vec<_>>())
        .unwrap_or_default();
    assert_eq!(items.len(), 2, "{}", parse.debug_tree());
    let second_stmt = items[1].syntax();
    assert_eq!(second_stmt.kind(), SyntaxKind::StmtLet);
}

#[test]
fn compact_snapshot_for_recovery_matrix() {
    let parse = parse_text(
        r##"fn broken(x, { return x; }
let sum = 1 + ;
let map = #{ a: 1 b: 2 };
let invoke = run(1 2);"##,
    );

    let expected = r##"Root
  RootItemList
    ItemFn
      FnKw "fn"
      Ident "broken"
      ParamList
        OpenParen "("
        Ident "x"
        Comma ","
        Error
        Error
      Block
        OpenBrace "{"
        BlockItemList
          StmtReturn
            ReturnKw "return"
            ExprName
              Ident "x"
            Semicolon ";"
        CloseBrace "}"
    StmtLet
      LetKw "let"
      Ident "sum"
      Eq "="
      ExprBinary
        ExprLiteral
          Int "1"
        Plus "+"
        Error
      Semicolon ";"
    StmtLet
      LetKw "let"
      Ident "map"
      Eq "="
      ExprObject
        HashBraceOpen "#{"
        ObjectFieldList
          ObjectField
            Ident "a"
            Colon ":"
            ExprLiteral
              Int "1"
          Error
          ObjectField
            Ident "b"
            Colon ":"
            ExprLiteral
              Int "2"
        CloseBrace "}"
      Semicolon ";"
    StmtLet
      LetKw "let"
      Ident "invoke"
      Eq "="
      ExprCall
        ExprName
          Ident "run"
        ArgList
          OpenParen "("
          ExprLiteral
            Int "1"
          Error
          ExprLiteral
            Int "2"
          CloseParen ")"
      Semicolon ";"
"##;

    assert_eq!(parse.debug_tree_compact(), expected);
}
