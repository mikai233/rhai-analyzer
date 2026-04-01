use crate::FormatOptions;
use crate::tests::range::{assert_range_rewrites_to, default_range, prefer_multiline_options};

#[test]
fn range_formatter_can_target_parameter_lists() {
    let source = "fn run(alpha,beta,gamma,delta){delta}\n";
    let selection_start =
        u32::try_from(source.find("alpha").expect("expected params")).expect("offset");
    let selection_end =
        u32::try_from(source.find("){").expect("expected param list end")).expect("offset");

    assert_range_rewrites_to(
        source,
        default_range(selection_start, selection_end),
        &FormatOptions {
            max_line_length: 16,
            ..FormatOptions::default()
        },
        "fn run(\n    alpha,\n    beta,\n    gamma,\n    delta,\n){delta}\n",
    );
}

#[test]
fn range_formatter_can_target_nested_array_expressions() {
    let source = "fn run(){let values=[1,2,3];values}\n";
    let selection_start =
        u32::try_from(source.find("[").expect("expected array start")).expect("offset");
    let selection_end =
        u32::try_from(source.find("];").expect("expected array end") + 1).expect("offset");

    assert_range_rewrites_to(
        source,
        default_range(selection_start, selection_end),
        &prefer_multiline_options(),
        "fn run(){let values=[\n        1,\n        2,\n        3,\n    ];values}\n",
    );
}

#[test]
fn range_formatter_can_target_array_item_lists() {
    let source = "fn run(){let values=[alpha,beta,gamma,delta];}\n";
    let selection_start =
        u32::try_from(source.find("alpha").expect("expected array items")).expect("offset");
    let selection_end =
        u32::try_from(source.find("];").expect("expected array end")).expect("offset");

    assert_range_rewrites_to(
        source,
        default_range(selection_start, selection_end),
        &FormatOptions {
            max_line_length: 18,
            ..FormatOptions::default()
        },
        "fn run(){let values=[\n        alpha,\n        beta,\n        gamma,\n        delta,\n    ];}\n",
    );
}

#[test]
fn range_formatter_can_target_interpolation_item_lists() {
    let source = "let message = `value=${let total=foo+bar;total}`;\n";
    let selection_start = u32::try_from(
        source
            .find("let total")
            .expect("expected interpolation body"),
    )
    .expect("offset");
    let selection_end =
        u32::try_from(source.find("}").expect("expected interpolation end")).expect("offset");

    assert_range_rewrites_to(
        source,
        default_range(selection_start, selection_end),
        &FormatOptions {
            max_line_length: 30,
            ..FormatOptions::default()
        },
        "let message = `value=${\n    let total = foo + bar;\n    total\n}`;\n",
    );
}

#[test]
fn range_formatter_can_target_string_part_lists() {
    let source = "let message = `hello ${foo+bar} world ${baz+qux}`;\n";
    let selection_start =
        u32::try_from(source.find("hello").expect("expected string part body")).expect("offset");
    let selection_end =
        u32::try_from(source.rfind("}").expect("expected string part end") + 1).expect("offset");

    assert_range_rewrites_to(
        source,
        default_range(selection_start, selection_end),
        &FormatOptions::default(),
        "let message = `hello ${foo + bar} world ${baz + qux}`;\n",
    );
}

#[test]
fn range_formatter_can_target_object_field_lists() {
    let source =
        "fn run(){let user=#{first_name:\"Ada\",last_name:\"Lovelace\",city:\"London\"};}\n";
    let selection_start =
        u32::try_from(source.find("first_name").expect("expected object fields")).expect("offset");
    let selection_end =
        u32::try_from(source.find("};").expect("expected object end")).expect("offset");

    assert_range_rewrites_to(
        source,
        default_range(selection_start, selection_end),
        &FormatOptions {
            max_line_length: 26,
            ..FormatOptions::default()
        },
        "fn run(){let user=#{\n        first_name: \"Ada\",\n        last_name: \"Lovelace\",\n        city: \"London\",\n};}\n",
    );
}

#[test]
fn range_formatter_can_target_call_argument_lists() {
    let source = "fn run(){helper(alpha,beta,gamma,delta);}\n";
    let selection_start =
        u32::try_from(source.find("alpha").expect("expected args")).expect("offset");
    let selection_end =
        u32::try_from(source.find(");").expect("expected call end")).expect("offset");

    assert_range_rewrites_to(
        source,
        default_range(selection_start, selection_end),
        &FormatOptions {
            max_line_length: 18,
            ..FormatOptions::default()
        },
        "fn run(){helper(\n        alpha,\n        beta,\n        gamma,\n        delta,\n    );}\n",
    );
}

#[test]
fn range_formatter_can_target_switch_pattern_lists() {
    let source = "fn run(kind){switch kind {foo|bar=>1}}\n";
    let selection_start =
        u32::try_from(source.find("foo").expect("expected pattern start")).expect("offset");
    let selection_end =
        u32::try_from(source.find("=>").expect("expected arrow start")).expect("offset");

    assert_range_rewrites_to(
        source,
        default_range(selection_start, selection_end),
        &FormatOptions::default(),
        "fn run(kind){switch kind {foo | bar=>1}}\n",
    );
}

#[test]
fn range_formatter_can_target_switch_arm_lists() {
    let source = "fn run(kind){switch kind {foo=>alpha+beta,bar=>gamma+delta}}\n";
    let selection_start =
        u32::try_from(source.find("foo").expect("expected switch arms")).expect("offset");
    let selection_end =
        u32::try_from(source.find("}").expect("expected switch end")).expect("offset");

    assert_range_rewrites_to(
        source,
        default_range(selection_start, selection_end),
        &FormatOptions::default(),
        "fn run(kind){switch kind {foo => alpha + beta, bar => gamma + delta}}\n",
    );
}
