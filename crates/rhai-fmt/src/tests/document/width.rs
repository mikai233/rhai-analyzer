use crate::FormatOptions;

use crate::tests::assert_formats_to_with_options;

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
#[test]
fn formatter_wraps_long_control_flow_heads_under_width_constraints() {
    let source = r#"
fn run(items){
if module::service::profile { step() } else { fallback() }
while module::service::ready { tick() }
for (very_long_item_name,very_long_index_name) in module::service::items { consume(very_long_item_name,very_long_index_name); }
do { tick() } until module::service::ready
}
"#;

    let expected = r#"fn run(items) {
    if module::service
            ::profile {
        step()
    } else {
        fallback()
    }
    while module::service
            ::ready {
        tick()
    }
    for
        (
            very_long_item_name,
            very_long_index_name
        )
        in module::service
            ::items {
        consume(
            very_long_item_name,
            very_long_index_name,
        );
    }
    do {
        tick()
    } until
        module::service::ready
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
#[test]
fn formatter_wraps_long_closure_heads_under_width_constraints() {
    let source = r#"
fn run(){
let mapper=|very_long_value_name| value;
}
"#;

    let expected = r#"fn run() {
    let mapper
        = |very_long_value_name|
            value;
}
"#;

    assert_formats_to_with_options(
        source,
        expected,
        &FormatOptions {
            max_line_length: 34,
            ..FormatOptions::default()
        },
    );
}
#[test]
fn formatter_wraps_long_function_signatures_under_width_constraints() {
    let source = r#"
private fn "Custom-Type".very_long_method_name(left,right,third){left+right+third}
"#;

    let expected = r#"private
fn
"Custom-Type".very_long_method_name(left, right, third) {
    left + right + third
}
"#;

    assert_formats_to_with_options(
        source,
        expected,
        &FormatOptions {
            max_line_length: 22,
            ..FormatOptions::default()
        },
    );
}
