use crate::{AstNode, Item, Root, SyntaxKind, parse_text};

#[test]
fn parses_function_items_with_private_modifier() {
    let parse = parse_text("private fn add(x, y,) { return x + y; }");

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let root = Root::cast(parse.root()).expect("expected root");
    let item = root
        .item_list()
        .and_then(|items| items.items().next())
        .map(Item::syntax)
        .expect("expected item node");
    assert_eq!(item.kind(), SyntaxKind::ItemFn);

    let tree = parse.debug_tree();
    assert!(tree.contains("ParamList"), "{tree}");
    assert!(tree.contains("StmtReturn"), "{tree}");
}

#[test]
fn parses_typed_method_function_definitions() {
    let parse = parse_text(
        r#"
        fn int.do_update(x, y) { this += x + y; }
        fn "Custom-Type".refresh() { this = 1; }
    "#,
    );

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let tree = parse.debug_tree();
    assert!(tree.contains("ItemFn"), "{tree}");
    assert!(tree.contains("String \"\\\"Custom-Type\\\"\""), "{tree}");
    assert!(tree.contains("Dot \".\""), "{tree}");
}

#[test]
fn parses_caller_scope_function_calls() {
    let parse = parse_text(r#"let value = helper!(1); let other = call!(worker, 2);"#);

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let tree = parse.debug_tree();
    assert!(tree.contains("Bang \"!\""), "{tree}");
    assert!(tree.matches("ExprCall").count() >= 2, "{tree}");
    assert!(tree.contains("CallKw \"call\""), "{tree}");
}

#[test]
fn caller_scope_calls_reject_method_and_path_forms() {
    let parse = parse_text(r#"object.method!(); mod::func!();"#);

    let messages = parse
        .errors()
        .iter()
        .map(|error| error.message())
        .collect::<Vec<_>>();
    assert!(
        messages.contains(&"caller-scope function calls cannot use method-call style"),
        "{}",
        parse.debug_tree()
    );
    assert!(
        messages.contains(&"caller-scope function calls cannot use namespace-qualified paths"),
        "{}",
        parse.debug_tree()
    );
}

#[test]
fn parses_elvis_method_calls_in_function_call_style() {
    let parse = parse_text(r#"let value = object?.method("home", 42);"#);

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let tree = parse.debug_tree();
    assert!(tree.contains("ExprCall"), "{tree}");
    assert!(tree.contains("ExprField"), "{tree}");
    assert!(tree.contains("QuestionDot \"?.\""), "{tree}");
    assert!(tree.contains("Ident \"method\""), "{tree}");
}

#[test]
fn typed_method_receiver_requires_dot_name_shape() {
    let parse = parse_text(r#"fn "Custom-Type"() { this; }"#);

    let messages = parse
        .errors()
        .iter()
        .map(|error| error.message())
        .collect::<Vec<_>>();
    assert!(
        messages.contains(&"expected `.` after typed method receiver"),
        "{}",
        parse.debug_tree()
    );
}

#[test]
fn function_definitions_are_restricted_to_global_level() {
    let parse = parse_text(
        r#"
        fn outer() {
            fn inner() {}
        }
    "#,
    );

    let messages = parse
        .errors()
        .iter()
        .map(|error| error.message())
        .collect::<Vec<_>>();
    assert!(
        messages.contains(&"functions can only be defined at global level"),
        "{}",
        parse.debug_tree()
    );
}

#[test]
fn parses_const_import_export_and_paths() {
    let parse = parse_text(
        r#"
        const ANSWER = 42;
        import "crypto" as secure;
        export ANSWER as exported_answer;
        let hashed = global::crypto::sha256(secure::seed);
    "#,
    );

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let tree = parse.debug_tree();
    assert!(tree.contains("StmtConst"), "{tree}");
    assert!(tree.contains("StmtImport"), "{tree}");
    assert!(tree.contains("StmtExport"), "{tree}");
    assert!(tree.contains("AliasClause"), "{tree}");
    assert!(tree.contains("ExprPath"), "{tree}");
}

#[test]
fn parses_export_shorthand_declarations() {
    let parse = parse_text(
        r#"
        export const ANSWER = 42;
        export let count = 1;
    "#,
    );

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let tree = parse.debug_tree();
    assert!(tree.contains("StmtExport"), "{tree}");
    assert!(tree.contains("StmtConst"), "{tree}");
    assert!(tree.contains("StmtLet"), "{tree}");
}

#[test]
fn export_rejects_non_global_targets_and_path_expressions() {
    let parse = parse_text(
        r#"
        let value = 1;
        export global::value as answer;
        if true { export value; }
    "#,
    );

    let messages = parse
        .errors()
        .iter()
        .map(|error| error.message())
        .collect::<Vec<_>>();
    assert!(
        messages.contains(
            &"expected exported variable name or `let`/`const` declaration after `export`"
        ),
        "{}",
        parse.debug_tree()
    );
    assert!(
        messages.contains(&"the `export` statement can only be used at global level"),
        "{}",
        parse.debug_tree()
    );
}
