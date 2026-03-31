use crate::{ContainerLayoutStyle, FormatOptions, ImportSortOrder, format_range, format_text};
use rhai_syntax::{TextRange, TextSize};

use crate::tests::{apply_range_edit, assert_idempotent_with_options, assert_parse_stable};

#[test]
fn formatter_is_idempotent_across_representative_documents() {
    let cases = [
        r#"
import "pkg" as pkg;

fn run(value){
let values=[1,2,3];
let user=#{name:"Ada",scores:[1,2]};
if value>0{
pkg::boot(values[0],user.name.len());
} else {
switch value{0=>`zero`,_=>`other`}
}
}
"#,
        r#"
/// docs
fn render(user,items){
// before
let safe=user?.profile?[index+1].name;
let summary=`user=${user?.profile.name} total=${items.len()+1}`;
let detailed=`result=${let total=items.len()+1;
if total>10 { `many:${total}` } else { `few:${total}` }}`;
detailed
}
"#,
        r#"
fn flow(items){
let mapper=|value| value+1;
for (item,index) in items { while index<10 { if item>0 { break item; } else { continue; } } }
do { mapper(items[0]) } while items.len()<3;
}
"#,
    ];

    for case in cases {
        assert_idempotent_with_options(case, &FormatOptions::default());
    }
}

#[test]
fn formatter_is_idempotent_under_non_default_policy_combinations() {
    let source = r#"
import "zebra" as zebra;
import "alpha";

fn run(left,right){
let values=[1,2,3];
let user=#{name:"Ada",scores:[1,2]};
helper(left,right,values,user)
}
"#;

    assert_idempotent_with_options(
        source,
        &FormatOptions {
            indent_style: crate::IndentStyle::Tabs,
            indent_width: 2,
            max_line_length: 80,
            trailing_commas: false,
            final_newline: false,
            container_layout: ContainerLayoutStyle::PreferMultiLine,
            import_sort_order: ImportSortOrder::ModulePath,
            ..FormatOptions::default()
        },
    );
}

#[test]
fn range_formatting_edits_produce_parse_stable_and_fully_stable_documents() {
    let source = "import \"tools\" as tools;\n\nfn run(){let values=[1,2,3]; tools::add(values[0],value+1)}\n";
    let range_start = TextSize::from(
        u32::try_from(source.find("fn run").expect("expected function start")).expect("offset"),
    );
    let range_end = TextSize::from(source.len() as u32);
    let range = TextRange::new(range_start, range_end);

    let result =
        format_range(source, range, &FormatOptions::default()).expect("expected range edit");
    let rewritten = apply_range_edit(source, result.range, &result.text);

    assert_parse_stable(&rewritten);

    let formatted = format_text(&rewritten, &FormatOptions::default());
    assert!(
        !formatted.changed,
        "expected full-document formatter to agree after range edit\nrewritten:\n{}\nfull:\n{}",
        rewritten, formatted.text
    );
}

#[test]
fn formatter_leaves_parse_error_documents_unchanged() {
    let source = "fn run( {\nlet value =\n";

    let result = format_text(source, &FormatOptions::default());

    assert_eq!(result.text, source);
    assert!(!result.changed);
}

#[test]
fn range_formatter_returns_none_for_parse_error_documents() {
    let source = "fn run( {\nlet value =\n";
    let range = TextRange::new(TextSize::from(0), TextSize::from(source.len() as u32));

    assert!(format_range(source, range, &FormatOptions::default()).is_none());
}
