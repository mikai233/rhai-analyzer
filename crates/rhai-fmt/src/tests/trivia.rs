use crate::tests::assert_formats_to;

#[test]
fn formatter_preserves_top_level_doc_comments() {
    let source = r#"
/// Adds one to the value.
fn add_one(value){
value+1
}
"#;

    let expected = r#"/// Adds one to the value.
fn add_one(value) {
    value + 1
}
"#;

    assert_formats_to(source, expected);
}

#[test]
fn formatter_keeps_blank_line_between_functions_and_following_doc_comments() {
    let source = r#"
fn first(){
1
}
/// second docs
fn second(){
2
}
"#;

    let expected = r#"fn first() {
    1
}

/// second docs
fn second() {
    2
}
"#;

    assert_formats_to(source, expected);
}

#[test]
fn formatter_preserves_extra_blank_lines_between_top_level_items() {
    let source = r#"
fn first(){
1
}


fn second(){
2
}
"#;

    let expected = r#"fn first() {
    1
}


fn second() {
    2
}
"#;

    assert_formats_to(source, expected);
}

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
