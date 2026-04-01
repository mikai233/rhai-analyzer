use crate::{SyntaxErrorCode, parse_text};

#[test]
fn interpolation_body_parser_errors_use_absolute_ranges() {
    let source = "let msg = `value = ${1 + }`;";
    let parse = parse_text(source);
    let error = parse
        .errors()
        .iter()
        .find(|error| error.code() == &SyntaxErrorCode::ExpectedExpressionAfterOperator)
        .expect("expected interpolation body parser error");

    let start = u32::from(error.range().start()) as usize;
    assert!(start > source.find("${").expect("expected interpolation start"));
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
