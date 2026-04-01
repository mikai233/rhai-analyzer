use crate::{SyntaxErrorCode, parse_text};

#[test]
fn reports_missing_semicolon_between_non_terminal_statements() {
    let parse = parse_text(
        r#"
        let v = "hello"
        let q = 1.0 + 2;
    "#,
    );

    let codes = parse
        .errors()
        .iter()
        .map(|error| error.code())
        .collect::<Vec<_>>();
    assert!(
        codes.contains(&&SyntaxErrorCode::ExpectedSemicolonToTerminateStatement),
        "{}",
        parse.debug_tree()
    );
}

#[test]
fn reports_missing_semicolon_after_let_even_when_initializer_is_block() {
    let parse = parse_text(
        r#"
        let value = { 40 + 2 }
        let other = 1;
    "#,
    );

    let codes = parse
        .errors()
        .iter()
        .map(|error| error.code())
        .collect::<Vec<_>>();
    assert!(
        codes.contains(&&SyntaxErrorCode::ExpectedSemicolonToTerminateStatement),
        "{}",
        parse.debug_tree()
    );
}

#[test]
fn allows_block_ending_statements_to_omit_semicolons_before_next_statement() {
    let parse = parse_text(
        r#"
        if true { 1 }
        let other = 1;
    "#,
    );

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());
}
