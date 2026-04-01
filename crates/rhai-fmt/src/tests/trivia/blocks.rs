use crate::tests::assert_formats_to;

#[test]
fn formatter_preserves_comments_inside_blocks() {
    let source = r#"
fn run(){
// before
let value=1;
// after
value
/* trailing */
}
"#;

    let expected = r#"fn run() {
    // before
    let value = 1;
    // after
    value
    /* trailing */
}
"#;

    assert_formats_to(source, expected);
}
#[test]
fn formatter_preserves_dangling_comments_in_empty_blocks() {
    let source = r#"
fn run(){
if ready{
// waiting
}
}
"#;

    let expected = r#"fn run() {
    if ready {
        // waiting
    }
}
"#;

    assert_formats_to(source, expected);
}
#[test]
fn formatter_preserves_extra_blank_lines_inside_blocks() {
    let source = r#"
fn run(){
let first=1;


let second=2;
second
}
"#;

    let expected = r#"fn run() {
    let first = 1;


    let second = 2;
    second
}
"#;

    assert_formats_to(source, expected);
}
#[test]
fn formatter_keeps_trailing_line_comments_on_statements() {
    let source = r#"
fn run(){
let value=1; // trailing
value // tail expr
}
"#;

    let expected = r#"fn run() {
    let value = 1; // trailing
    value // tail expr
}
"#;

    assert_formats_to(source, expected);
}
#[test]
fn formatter_keeps_trailing_comments_before_following_leading_comments() {
    let source = r#"
fn first(){1} // trailing first
/// docs for second
fn second(){2}
"#;

    let expected = r#"fn first() {
    1
} // trailing first

/// docs for second
fn second() {
    2
}
"#;

    assert_formats_to(source, expected);
}
