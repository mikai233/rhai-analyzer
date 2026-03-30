use crate::{FormatMode, FormatOptions, IndentStyle, format_range, format_text};
use rhai_syntax::{TextRange, TextSize, parse_text};

#[test]
fn formatter_rewrites_basic_functions_blocks_and_binary_spacing() {
    let source = r#"
fn helper(value){
let result=value+1;
result
}
"#;

    let result = format_text(source, &FormatOptions::default());
    let expected = r#"fn helper(value) {
    let result = value + 1;
    result
}
"#;

    assert_eq!(result.text, expected);
    assert!(result.changed);
    assert!(parse_text(&result.text).errors().is_empty());
}

#[test]
fn formatter_rewrites_arrays_objects_calls_and_imports() {
    let source = r#"import   "tools"  as tools;
fn run(){let values=[1,2,3]; let user=#{name:"Ada",scores:[1,2]}; tools::add(values[0],user.name.len());}
"#;

    let result = format_text(source, &FormatOptions::default());
    let expected = r#"import "tools" as tools;

fn run() {
    let values = [1, 2, 3];
    let user = #{name: "Ada", scores: [1, 2]};
    tools::add(values[0], user.name.len());
}
"#;

    assert_eq!(result.text, expected);
    assert!(parse_text(&result.text).errors().is_empty());
}

#[test]
fn formatter_preserves_tail_expressions_and_multiline_containers() {
    let source = r#"
fn build(){
if true{#{name:"Ada",numbers:[1,2,3,4,5,6,7,8,9,10,11,12]}} else {#{name:"Bob"}}
}
"#;

    let result = format_text(source, &FormatOptions::default());
    let expected = r#"fn build() {
    if true {
        #{
            name: "Ada",
            numbers: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
        }
    } else {
        #{name: "Bob"}
    }
}
"#;

    assert_eq!(result.text, expected);
    assert!(parse_text(&result.text).errors().is_empty());
}

#[test]
fn formatter_rewrites_switch_loops_and_closures() {
    let source = r#"
fn flow(items){
let mapper=|value| value+1;
let label=switch items.len(){0=>`empty`,1|2=>`small`,_=>{`many`}};
for (item,index) in items { while index<10 { if item>0 { break item; } else { continue; } } }
loop { break; }
do { mapper(items[0]) } while items.len()<3;
}
"#;

    let result = format_text(source, &FormatOptions::default());
    let expected = r#"fn flow(items) {
    let mapper = |value| value + 1;
    let label = switch items.len() {
        0 => `empty`,
        1 | 2 => `small`,
        _ => {
            `many`
        }
    };
    for (item, index) in items {
        while index < 10 {
            if item > 0 {
                break item;
            } else {
                continue;
            }
        }
    }
    loop {
        break;
    }
    do {
        mapper(items[0])
    } while items.len() < 3;
}
"#;

    assert_eq!(result.text, expected);
    assert!(parse_text(&result.text).errors().is_empty());
}

#[test]
fn formatter_preserves_top_level_doc_comments() {
    let source = r#"
/// Adds one to the value.
fn add_one(value){
value+1
}
"#;

    let result = format_text(source, &FormatOptions::default());
    let expected = r#"/// Adds one to the value.
fn add_one(value) {
    value + 1
}
"#;

    assert_eq!(result.text, expected);
    assert!(parse_text(&result.text).errors().is_empty());
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

    let result = format_text(source, &FormatOptions::default());
    let expected = r#"fn first() {
    1
}

/// second docs
fn second() {
    2
}
"#;

    assert_eq!(result.text, expected);
    assert!(parse_text(&result.text).errors().is_empty());
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

    let result = format_text(source, &FormatOptions::default());
    let expected = r#"fn first() {
    1
}


fn second() {
    2
}
"#;

    assert_eq!(result.text, expected);
    assert!(parse_text(&result.text).errors().is_empty());
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

    let result = format_text(source, &FormatOptions::default());
    let expected = r#"fn run() {
    // before
    let value = 1;
    // after
    value
    /* trailing */
}
"#;

    assert_eq!(result.text, expected);
    assert!(parse_text(&result.text).errors().is_empty());
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

    let result = format_text(source, &FormatOptions::default());
    let expected = r#"fn run() {
    let first = 1;


    let second = 2;
    second
}
"#;

    assert_eq!(result.text, expected);
    assert!(parse_text(&result.text).errors().is_empty());
}

#[test]
fn formatter_options_default_to_document_spaces_and_trailing_commas() {
    let options = FormatOptions::default();

    assert_eq!(options.mode, FormatMode::Document);
    assert_eq!(options.indent_style, IndentStyle::Spaces);
    assert_eq!(options.indent_width, 4);
    assert_eq!(options.max_line_length, 100);
    assert!(options.trailing_commas);
}

#[test]
fn range_formatter_returns_minimal_changed_region_when_selection_intersects() {
    let source = "fn run(){let value=1+2;value}\n";
    let range = TextRange::new(TextSize::from(0), TextSize::from(source.len() as u32));

    let result = format_range(source, range, &FormatOptions::default())
        .expect("expected range formatting result");

    assert!(u32::from(result.range.start()) < u32::from(result.range.end()));
    assert!(result.text.contains("let value = 1 + 2;"));
    assert!(result.text.contains("value"));
}

#[test]
fn range_formatter_returns_none_when_selection_does_not_intersect_change() {
    let source = "fn run(){let value=1+2;value}\n";
    let untouched_tail = TextRange::new(TextSize::from(28), TextSize::from(29));

    assert!(format_range(source, untouched_tail, &FormatOptions::default()).is_none());
}
