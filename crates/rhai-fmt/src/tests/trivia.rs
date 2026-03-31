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
fn formatter_preserves_comments_between_function_signature_and_body() {
    let source = r#"
fn run(value)
// explain body
{
value
}
"#;

    let expected = r#"fn run(value)
// explain body
{
    value
}
"#;

    assert_formats_to(source, expected);
}

#[test]
fn formatter_preserves_comments_inside_function_signatures() {
    let source = r#"
private /* keep private */ fn /* keep fn */ "Custom-Type" /* keep dot */ . /* keep name */ refresh /* keep params */ (value){
value
}
"#;

    let expected = r#"private /* keep private */ fn /* keep fn */ "Custom-Type" /* keep dot */ . /* keep name */ refresh /* keep params */ (value) {
    value
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
fn formatter_preserves_dangling_comments_after_last_delimited_item() {
    let source = r#"
fn run(){
process(
value,
// keep trailing arg comment
);
}
"#;

    let expected = r#"fn run() {
    process(
        value,
        // keep trailing arg comment
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
fn formatter_preserves_comments_between_while_or_loop_heads_and_bodies() {
    let source = r#"
fn run(value){
while value > 0
// loop while positive
{
value
}

loop
// keep polling
{
break;
}
}
"#;

    let expected = r#"fn run(value) {
    while value > 0
    // loop while positive
    {
        value
    }

    loop
    // keep polling
    {
        break;
    }
}
"#;

    assert_formats_to(source, expected);
}

#[test]
fn formatter_preserves_comments_between_if_for_do_heads_and_bodies() {
    let source = r#"
fn run(items, ready){
if ready
// enter branch
{
items.len()
} else
// nested branch
{
0
}

for item in items
// visit item
{
item
}

do
// once first
{
step()
}
// stop when ready
while ready
}
"#;

    let expected = r#"fn run(items, ready) {
    if ready
    // enter branch
    {
        items.len()
    } else
    // nested branch
    {
        0
    }

    for item in items
    // visit item
    {
        item
    }

    do
    // once first
    {
        step()
    }
    // stop when ready
    while ready
}
"#;

    assert_formats_to(source, expected);
}

#[test]
fn formatter_preserves_comments_between_try_catch_heads_and_bodies() {
    let source = r#"
fn run(){
try
// attempt work
{
work()
}
// recover below
catch (err)
// inspect error
{
log(err);
}
}
"#;

    let expected = r#"fn run() {
    try
    // attempt work
    {
        work()
    }
    // recover below
    catch (err)
    // inspect error
    {
        log(err);
    }
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
