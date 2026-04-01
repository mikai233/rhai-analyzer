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
