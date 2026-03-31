use rhai_syntax::{AstNode, Expr, Item, Root, Stmt, parse_text};

use crate::formatter::support::coverage::{FormatSupportLevel, expr_support};
use crate::tests::assert_formats_to;

#[test]
fn formatter_support_matrix_marks_comment_sensitive_exprs_structural() {
    let source = r#"
fn run() {
    value + 1;
    helper(alpha);
    items[0];
    items?[0];
    obj?.name;
    if ready { value } else { other };
    switch mode { _ => value };
    `hello ${name}`;
}
"#;

    let parse = parse_text(source);
    let root = Root::cast(parse.root()).expect("expected root");
    let function = root
        .items()
        .find_map(|item| match item {
            Item::Fn(function) => Some(function),
            Item::Stmt(_) => None,
        })
        .expect("expected function item");
    let body = function.body().expect("expected function body");

    let levels = body
        .items()
        .filter_map(|item| match item {
            Item::Stmt(Stmt::Expr(expr_stmt)) => expr_stmt.expr(),
            _ => None,
        })
        .map(|expr| (expr, expr_support(expr).level))
        .collect::<Vec<_>>();

    assert_eq!(levels.len(), 8);
    assert!(matches!(levels[0].0, Expr::Binary(_)));
    assert_eq!(levels[0].1, FormatSupportLevel::Structural);
    assert!(matches!(levels[1].0, Expr::Call(_)));
    assert_eq!(levels[1].1, FormatSupportLevel::Structural);
    assert!(matches!(levels[2].0, Expr::Index(_)));
    assert_eq!(levels[2].1, FormatSupportLevel::Structural);
    assert!(matches!(levels[3].0, Expr::Index(_)));
    assert_eq!(levels[3].1, FormatSupportLevel::Structural);
    assert!(matches!(levels[4].0, Expr::Field(_)));
    assert_eq!(levels[4].1, FormatSupportLevel::Structural);
    assert!(matches!(levels[5].0, Expr::If(_)));
    assert_eq!(levels[5].1, FormatSupportLevel::Structural);
    assert!(matches!(levels[6].0, Expr::Switch(_)));
    assert_eq!(levels[6].1, FormatSupportLevel::Structural);
    assert!(matches!(levels[7].0, Expr::InterpolatedString(_)));
    assert_eq!(levels[7].1, FormatSupportLevel::Structural);
}

#[test]
fn formatter_formats_phase_four_safe_access_and_interpolated_strings() {
    let source = r#"
fn run(){
let safe = items?[index+1];
let name = user?.profile;
let message = `hello ${user?.name} ${value+1}`;
}
"#;

    let expected = r#"fn run() {
    let safe = items?[index + 1];
    let name = user?.profile;
    let message = `hello ${user?.name} ${value + 1}`;
}
"#;

    assert_formats_to(source, expected);
}

#[test]
fn formatter_falls_back_to_raw_for_comment_sensitive_expression_boundaries() {
    let source = r#"
fn run(){
let value=left /* keep */ + right;
let user=object /* keep */ .field;
let result=helper /* keep */ (value);
}
"#;

    let expected = r#"fn run() {
    let value = left /* keep */ + right;
    let user = object /* keep */ .field;
    let result = helper /* keep */ (value);
}
"#;

    assert_formats_to(source, expected);
}
