use rhai_syntax::{AstNode, Expr, Item, Root, Stmt, parse_text};

use crate::formatter::support::coverage::{
    FormatSupportLevel, expr_support, item_support, stmt_support,
};
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
        .item_list()
        .into_iter()
        .flat_map(|items| items.items())
        .find_map(|item| match item {
            Item::Fn(function) => Some(function),
            Item::Stmt(_) => None,
        })
        .expect("expected function item");
    let body = function.body().expect("expected function body");

    let levels = body
        .item_list()
        .into_iter()
        .flat_map(|items| items.items())
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
fn formatter_formats_comment_sensitive_expression_operator_boundaries() {
    let source = r#"
fn run(){
let value=(left+1) /* keep */ + (right+2);
target /* keep assign */ = source+1;
let neg=! /* keep unary */ value;
let wrapped=( /* keep paren */ value+1);
}
"#;

    let expected = r#"fn run() {
    let value = (left + 1) /* keep */ + (right + 2);
    target /* keep assign */ = source + 1;
    let neg = ! /* keep unary */ value;
    let wrapped = ( /* keep paren */ value + 1);
}
"#;

    assert_formats_to(source, expected);
}

#[test]
fn formatter_support_matrix_marks_items_and_statements_structural() {
    let source = r#"
private fn demo(value) {
    let local = value;
    work(value) /* keep */;
    continue;
}

let root_value = compute();
"#;

    let parse = parse_text(source);
    let root = Root::cast(parse.root()).expect("expected root");
    let items = root
        .item_list()
        .map(|items| items.items().collect::<Vec<_>>())
        .unwrap_or_default();

    assert!(matches!(items[0], Item::Fn(_)));
    assert_eq!(item_support(items[0]).level, FormatSupportLevel::Structural);
    assert!(matches!(items[1], Item::Stmt(Stmt::Let(_))));
    assert_eq!(item_support(items[1]).level, FormatSupportLevel::Structural);

    let function = match items[0] {
        Item::Fn(function) => function,
        Item::Stmt(_) => panic!("expected function"),
    };
    let body = function.body().expect("expected function body");
    let body_statements = body
        .item_list()
        .map(|items| {
            items
                .items()
                .filter_map(|item| match item {
                    Item::Stmt(stmt) => Some(stmt),
                    Item::Fn(_) => None,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    assert!(matches!(body_statements[0], Stmt::Let(_)));
    assert_eq!(
        stmt_support(body_statements[0]).level,
        FormatSupportLevel::Structural
    );
    assert!(matches!(body_statements[1], Stmt::Expr(_)));
    assert_eq!(
        stmt_support(body_statements[1]).level,
        FormatSupportLevel::Structural
    );
    assert!(matches!(body_statements[2], Stmt::Continue(_)));
    assert_eq!(
        stmt_support(body_statements[2]).level,
        FormatSupportLevel::Structural
    );
}

#[test]
fn formatter_formats_comment_sensitive_statement_boundaries() {
    let source = r#"
fn run(){
let value /* keep eq */ = /* keep rhs */ left+1 /* keep semi */;
return /* keep return */ value+1 /* keep return semi */;
throw /* keep throw */ error+1 /* keep throw semi */;
helper(value+1) /* keep expr semi */;
}
"#;

    let expected = r#"fn run() {
    let value /* keep eq */ = /* keep rhs */ left + 1 /* keep semi */;
    return /* keep return */ value + 1 /* keep return semi */;
    throw /* keep throw */ error + 1 /* keep throw semi */;
    helper(value + 1) /* keep expr semi */;
}
"#;

    assert_formats_to(source, expected);
}

#[test]
fn formatter_formats_comment_sensitive_suffix_and_binding_boundaries() {
    let source = r#"
fn run(){
let indexed = source /* keep recv */ [ /* keep open */ index+1 /* keep close */ ];
for (item /* keep comma */, /* keep second */ position) in items {
item+position
}
let mapper = |left /* keep pipe comma */, /* keep right */ right| /* keep body */ left+right;
}
"#;

    let expected = r#"fn run() {
    let indexed = source /* keep recv */ [ /* keep open */ index + 1 /* keep close */ ];
    for (item /* keep comma */, /* keep second */ position) in items {
        item + position
    }
    let mapper = |left /* keep pipe comma */, /* keep right */ right| /* keep body */ left + right;
}
"#;

    assert_formats_to(source, expected);
}

#[test]
fn formatter_formats_comment_sensitive_clause_and_container_boundaries() {
    let source = r#"
import "tools" /* keep import alias */ as /* keep import name */ tools;
export helper /* keep export alias */ as /* keep export name */ public_helper;

fn run(data){
let user=#{
name /* keep colon */: /* keep value */ data.name,
};
let result=switch data.kind{
alpha /* keep arrow */ => /* keep value */ 1,
foo /* keep pipe */ | /* keep rhs */ bar /* keep arrow two */ => /* keep value two */ 2
};
while data.ready {
continue /* keep continue */;
}
result
}
"#;

    let expected = r#"import "tools" /* keep import alias */ as /* keep import name */ tools;

export helper /* keep export alias */ as /* keep export name */ public_helper;

fn run(data) {
    let user = #{name /* keep colon */: /* keep value */ data.name};
    let result = switch data.kind {
        alpha /* keep arrow */ => /* keep value */ 1,
        foo /* keep pipe */ | /* keep rhs */ bar /* keep arrow two */ => /* keep value two */ 2
    };
    while data.ready {
        continue /* keep continue */;
    }
    result
}
"#;

    assert_formats_to(source, expected);
}
