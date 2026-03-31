use crate::tests::{assert_formats_to, assert_formats_to_with_options};
use crate::{FormatOptions, ImportSortOrder};

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

#[test]
fn formatter_preserves_comments_inside_delimited_containers() {
    let source = r#"
fn run(
left,
// keep right
right,
){
let values=[
1, // first
// keep second
2,
];
let user=#{
name:"Ada",
// keep age
age:42,
};
process(
left, // keep left
// keep arg
right,
);
}
"#;

    let expected = r#"fn run(
    left,
    // keep right
    right,
) {
    let values = [
        1, // first
        // keep second
        2,
    ];
    let user = #{
        name: "Ada",
        // keep age
        age: 42,
    };
    process(
        left, // keep left
        // keep arg
        right,
    );
}
"#;

    assert_formats_to(source, expected);
}

#[test]
fn formatter_preserves_dangling_comments_in_empty_delimited_containers() {
    let source = r#"
fn run(){
let values=[
// nothing yet
];
let user=#{
// nothing here
};
process(
// no args
);
}
"#;

    let expected = r#"fn run() {
    let values = [
        // nothing yet
    ];
    let user = #{
        // nothing here
    };
    process(
        // no args
    );
}
"#;

    assert_formats_to(source, expected);
}

#[test]
fn formatter_preserves_comments_inside_switch_arms() {
    let source = r#"
fn run(value){
let label=switch value{
// zero
0=>1, // trailing zero

// one
1=>{
// nested
value
},
// fallback
_=>2
};
label
}
"#;

    let expected = r#"fn run(value) {
    let label = switch value {
        // zero
        0 => 1, // trailing zero

        // one
        1 => {
            // nested
            value
        },
        // fallback
        _ => 2
    };
    label
}
"#;

    assert_formats_to(source, expected);
}

#[test]
fn formatter_does_not_reorder_imports_when_comments_protect_their_boundary() {
    let source = r#"
import "zebra" as zebra;
// keep zebra first for override order
import "alpha" as alpha;
fn run(){ zebra::boot(); alpha::boot(); }
"#;

    let expected = r#"import "zebra" as zebra;
// keep zebra first for override order
import "alpha" as alpha;

fn run() {
    zebra::boot();
    alpha::boot();
}
"#;

    assert_formats_to_with_options(
        source,
        expected,
        &FormatOptions {
            import_sort_order: ImportSortOrder::ModulePath,
            ..FormatOptions::default()
        },
    );
}

#[test]
fn formatter_preserves_multiline_block_comment_layout() {
    let source = "fn run(){\n/*\n * aligned block\n * comment\n */\nvalue\n}\n";

    let expected = "fn run() {\n    /*\n * aligned block\n * comment\n */\n    value\n}\n";

    assert_formats_to(source, expected);
}

#[test]
fn formatter_preserves_multiline_doc_block_comment_layout() {
    let source = "/**\n * documented API\n * keeps star alignment\n */\nfn run(){value}\n";

    let expected =
        "/**\n * documented API\n * keeps star alignment\n */\nfn run() {\n    value\n}\n";

    assert_formats_to(source, expected);
}
