use crate::{ContainerLayoutStyle, FormatOptions};

use crate::tests::{assert_formats_to, assert_formats_to_with_options};

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
fn formatter_respects_skip_directive_for_next_statement() {
    let source = r#"fn run() {
    // rhai-fmt: skip
    let  weird   =#{ name :"Ada", values :[1,2,3]};
    let normal=1+2;
}
"#;

    let expected = r#"fn run() {
    // rhai-fmt: skip
    let  weird   =#{ name :"Ada", values :[1,2,3]};
    let normal = 1 + 2;
}
"#;

    assert_formats_to(source, expected);
}
