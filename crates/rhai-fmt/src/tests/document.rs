use crate::{ContainerLayoutStyle, FormatOptions, ImportSortOrder, format_text};

use crate::tests::{assert_formats_to, assert_formats_to_with_options};

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
}

#[test]
fn formatter_rewrites_arrays_objects_calls_and_imports() {
    let source = r#"import   "tools"  as tools;
fn run(){let values=[1,2,3]; let user=#{name:"Ada",scores:[1,2]}; tools::add(values[0],user.name.len());}
"#;

    let expected = r#"import "tools" as tools;

fn run() {
    let values = [1, 2, 3];
    let user = #{name: "Ada", scores: [1, 2]};
    tools::add(values[0], user.name.len());
}
"#;

    assert_formats_to(source, expected);
}

#[test]
fn formatter_preserves_tail_expressions_and_multiline_containers() {
    let source = r#"
fn build(){
if true{#{name:"Ada",numbers:[1,2,3,4,5,6,7,8,9,10,11,12]}} else {#{name:"Bob"}}
}
"#;

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

    assert_formats_to(source, expected);
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

    assert_formats_to(source, expected);
}

#[test]
fn formatter_rewrites_safe_access_chains_and_multiline_interpolations() {
    let source = r#"
fn render(user,items){
let safe=user?.profile?[index+1].name;
let summary=`user=${user?.profile.name} total=${items.len()+1}`;
let detailed=`result=${let total=items.len()+1;
if total>10 { `many:${total}` } else { `few:${total}` }}`;
}
"#;

    let expected = r#"fn render(user, items) {
    let safe = user?.profile?[index + 1].name;
    let summary = `user=${user?.profile.name} total=${items.len() + 1}`;
    let detailed = `result=${
        let total = items.len() + 1;
        if total > 10 {
            `many:${total}`
        } else {
            `few:${total}`
        }
    }`;
}
"#;

    assert_formats_to(source, expected);
}

#[test]
fn formatter_normalizes_import_export_sections() {
    let source = r#"import   "pkg"  as pkg;
import "tools";
export const CONFIG=#{name:"Ada",values:[1,2,3,4,5,6,7,8,9,10,11,12]};
export   helper   as public_helper;
fn run(){pkg::boot(); tools::boot();}
"#;

    let expected = r#"import "pkg" as pkg;
import "tools";

export const CONFIG = #{
    name: "Ada",
    values: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
};
export helper as public_helper;

fn run() {
    pkg::boot();
    tools::boot();
}
"#;

    assert_formats_to(source, expected);
}

#[test]
fn formatter_respects_final_newline_policy() {
    let source = "fn run(){let value=1+2;value}\n";
    let expected = "fn run() {\n    let value = 1 + 2;\n    value\n}";

    assert_formats_to_with_options(
        source,
        expected,
        &FormatOptions {
            final_newline: false,
            ..FormatOptions::default()
        },
    );
}

#[test]
fn formatter_can_prefer_multiline_containers_even_when_they_fit() {
    let source = "fn run(){let values=[1,2,3]; helper(alpha,beta);}\n";
    let expected = r#"fn run() {
    let values = [
        1,
        2,
        3,
    ];
    helper(
        alpha,
        beta,
    );
}
"#;

    assert_formats_to_with_options(
        source,
        expected,
        &FormatOptions {
            container_layout: ContainerLayoutStyle::PreferMultiLine,
            ..FormatOptions::default()
        },
    );
}

#[test]
fn formatter_can_prefer_single_line_objects_within_max_width() {
    let source =
        "fn run(){let user=#{first_name:\"Ada\",last_name:\"Lovelace\",city:\"London\"};}\n";
    let expected = "fn run() {\n    let user = #{first_name: \"Ada\", last_name: \"Lovelace\", city: \"London\"};\n}\n";

    assert_formats_to_with_options(
        source,
        expected,
        &FormatOptions {
            max_line_length: 100,
            container_layout: ContainerLayoutStyle::PreferSingleLine,
            ..FormatOptions::default()
        },
    );
}

#[test]
fn formatter_can_sort_top_level_import_runs_by_module_path() {
    let source = r#"import "zebra" as zebra;
import "alpha";
import "beta" as beta;
fn run(){}
"#;
    let expected = r#"import "alpha";
import "beta" as beta;
import "zebra" as zebra;

fn run() {}
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
fn formatter_preserves_blank_line_separated_import_groups_when_sorting() {
    let source = r#"import "zebra";
import "alpha";

import "delta";
import "beta";
fn run(){}
"#;
    let expected = r#"import "alpha";
import "zebra";

import "beta";
import "delta";

fn run() {}
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
fn formatter_formats_try_catch_bindings_with_parentheses() {
    let source = r#"
fn run(){
try { work(); } catch (err){ throw err; }
}
"#;

    let expected = r#"fn run() {
    try {
        work();
    } catch (err) {
        throw err;
    }
}
"#;

    assert_formats_to(source, expected);
}

#[test]
fn formatter_formats_private_typed_functions_and_caller_scope_calls() {
    let source = r#"
private fn int.do_update(left,right){helper!(left,right)}
fn "Custom-Type".refresh(){call!(worker,2);}
"#;

    let expected = r#"private fn int.do_update(left, right) {
    helper!(left, right)
}

fn "Custom-Type".refresh() {
    call!(worker, 2);
}
"#;

    assert_formats_to(source, expected);
}

#[test]
fn formatter_formats_do_until_loops() {
    let source = r#"
fn run(){
do { step() } until ready()
}
"#;

    let expected = r#"fn run() {
    do {
        step()
    } until ready()
}
"#;

    assert_formats_to(source, expected);
}

#[test]
fn formatter_wraps_long_binary_assignments_under_width_constraints() {
    let source = r#"
fn run(){
let total=alpha+beta+gamma+delta;
target=left+middle+right+tail;
}
"#;

    let expected = r#"fn run() {
    let total = alpha
        + beta
        + gamma
        + delta;
    target = left
        + middle
        + right
        + tail;
}
"#;

    assert_formats_to_with_options(
        source,
        expected,
        &FormatOptions {
            max_line_length: 20,
            ..FormatOptions::default()
        },
    );
}

#[test]
fn formatter_wraps_long_access_chains_under_width_constraints() {
    let source = r#"
fn run(){
let value=module::service::profile?[index+offset].display_name;
}
"#;

    let expected = r#"fn run() {
    let value = module
        ::service
        ::profile
        ?[index + offset]
        .display_name;
}
"#;

    assert_formats_to_with_options(
        source,
        expected,
        &FormatOptions {
            max_line_length: 24,
            ..FormatOptions::default()
        },
    );
}

#[test]
fn formatter_wraps_long_statement_heads_under_width_constraints() {
    let source = r#"
fn run(){
let outcome=module::service::profile;
return module::service::profile;
throw module::service::profile;
}
"#;

    let expected = r#"fn run() {
    let outcome
        = module
            ::service
            ::profile;
    return
        module::service::profile;
    throw
        module::service::profile;
}
"#;

    assert_formats_to_with_options(
        source,
        expected,
        &FormatOptions {
            max_line_length: 24,
            ..FormatOptions::default()
        },
    );
}

#[test]
fn formatter_wraps_long_switch_arm_values_under_width_constraints() {
    let source = r#"
fn run(mode){
switch mode { very_long_pattern_name => module::service::profile, _ => fallback() }
}
"#;

    let expected = r#"fn run(mode) {
    switch mode {
        very_long_pattern_name
            => module
                ::service
                ::profile,
        _ => fallback()
    }
}
"#;

    assert_formats_to_with_options(
        source,
        expected,
        &FormatOptions {
            max_line_length: 28,
            ..FormatOptions::default()
        },
    );
}
