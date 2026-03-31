use crate::tests::apply_range_edit;
use crate::{ContainerLayoutStyle, FormatOptions, format_range};
use rhai_syntax::{TextRange, TextSize};

#[test]
fn range_formatter_returns_minimal_changed_region_when_selection_intersects() {
    let source = "fn run(){let value=1+2;value}\n";
    let range = TextRange::new(TextSize::from(0), TextSize::from(source.len() as u32));

    let result = format_range(source, range, &FormatOptions::default())
        .expect("expected range formatting result");

    assert_eq!(u32::from(result.range.start()), 8);
    assert_eq!(u32::from(result.range.end()), 28);
    assert_eq!(
        result.text,
        " {\n    let value = 1 + 2;\n    value\n".to_owned()
    );
}

#[test]
fn range_formatter_returns_none_when_selection_does_not_intersect_change() {
    let source = "fn run(){let value=1+2;value}\n";
    let untouched_tail = TextRange::new(TextSize::from(28), TextSize::from(29));

    assert!(format_range(source, untouched_tail, &FormatOptions::default()).is_none());
}

#[test]
fn range_formatter_expands_to_enclosing_block_boundary() {
    let source = "fn run(){let value=1+2;value}\n";
    let selection_start =
        u32::try_from(source.find("value=1+2").expect("expected selection")).expect("offset");
    let trailing_value =
        u32::try_from(source.rfind("value").expect("expected trailing value")).expect("offset");
    let selection_end = trailing_value + "value".len() as u32;
    let range = TextRange::new(selection_start.into(), selection_end.into());

    let result = format_range(source, range, &FormatOptions::default())
        .expect("expected range formatting result");

    assert_eq!(
        apply_range_edit(source, result.range, &result.text),
        "fn run(){let value = 1 + 2;\n    value}\n"
    );
}

#[test]
fn range_formatter_can_target_parameter_lists() {
    let source = "fn run(alpha,beta,gamma,delta){delta}\n";
    let selection_start =
        u32::try_from(source.find("alpha").expect("expected params")).expect("offset");
    let selection_end =
        u32::try_from(source.find("){").expect("expected param list end")).expect("offset");
    let result = format_range(
        source,
        TextRange::new(selection_start.into(), selection_end.into()),
        &FormatOptions {
            max_line_length: 16,
            ..FormatOptions::default()
        },
    )
    .expect("expected range formatting result");

    assert_eq!(
        apply_range_edit(source, result.range, &result.text),
        "fn run(\n    alpha,\n    beta,\n    gamma,\n    delta,\n){delta}\n"
    );
}

#[test]
fn range_formatter_can_target_nested_array_expressions() {
    let source = "fn run(){let values=[1,2,3];values}\n";
    let selection_start =
        u32::try_from(source.find("[").expect("expected array start")).expect("offset");
    let selection_end =
        u32::try_from(source.find("];").expect("expected array end") + 1).expect("offset");
    let result = format_range(
        source,
        TextRange::new(selection_start.into(), selection_end.into()),
        &FormatOptions {
            container_layout: ContainerLayoutStyle::PreferMultiLine,
            ..FormatOptions::default()
        },
    )
    .expect("expected range formatting result");

    assert_eq!(
        apply_range_edit(source, result.range, &result.text),
        "fn run(){let values=[\n        1,\n        2,\n        3,\n    ];values}\n"
    );
}

#[test]
fn range_formatter_can_target_array_item_lists() {
    let source = "fn run(){let values=[alpha,beta,gamma,delta];}\n";
    let selection_start =
        u32::try_from(source.find("alpha").expect("expected array items")).expect("offset");
    let selection_end =
        u32::try_from(source.find("];").expect("expected array end")).expect("offset");
    let result = format_range(
        source,
        TextRange::new(selection_start.into(), selection_end.into()),
        &FormatOptions {
            max_line_length: 18,
            ..FormatOptions::default()
        },
    )
    .expect("expected range formatting result");

    assert_eq!(
        apply_range_edit(source, result.range, &result.text),
        "fn run(){let values=[\n        alpha,\n        beta,\n        gamma,\n        delta,\n    ];}\n"
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
    let result = format_range(
        source,
        TextRange::new(selection_start.into(), selection_end.into()),
        &FormatOptions {
            max_line_length: 30,
            ..FormatOptions::default()
        },
    )
    .expect("expected range formatting result");

    assert_eq!(
        apply_range_edit(source, result.range, &result.text),
        "let message = `value=${\n    let total = foo + bar;\n    total\n}`;\n"
    );
}

#[test]
fn range_formatter_can_target_string_part_lists() {
    let source = "let message = `hello ${foo+bar} world ${baz+qux}`;\n";
    let selection_start =
        u32::try_from(source.find("hello").expect("expected string part body")).expect("offset");
    let selection_end =
        u32::try_from(source.rfind("}").expect("expected string part end") + 1).expect("offset");
    let result = format_range(
        source,
        TextRange::new(selection_start.into(), selection_end.into()),
        &FormatOptions::default(),
    )
    .expect("expected range formatting result");

    assert_eq!(
        apply_range_edit(source, result.range, &result.text),
        "let message = `hello ${foo + bar} world ${baz + qux}`;\n"
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
    let result = format_range(
        source,
        TextRange::new(selection_start.into(), selection_end.into()),
        &FormatOptions {
            max_line_length: 26,
            ..FormatOptions::default()
        },
    )
    .expect("expected range formatting result");

    assert_eq!(
        apply_range_edit(source, result.range, &result.text),
        "fn run(){let user=#{\n        first_name: \"Ada\",\n        last_name: \"Lovelace\",\n        city: \"London\",\n};}\n"
    );
}

#[test]
fn range_formatter_can_target_call_argument_lists() {
    let source = "fn run(){helper(alpha,beta,gamma,delta);}\n";
    let selection_start =
        u32::try_from(source.find("alpha").expect("expected args")).expect("offset");
    let selection_end =
        u32::try_from(source.find(");").expect("expected call end")).expect("offset");
    let result = format_range(
        source,
        TextRange::new(selection_start.into(), selection_end.into()),
        &FormatOptions {
            max_line_length: 18,
            ..FormatOptions::default()
        },
    )
    .expect("expected range formatting result");

    assert_eq!(
        apply_range_edit(source, result.range, &result.text),
        "fn run(){helper(\n        alpha,\n        beta,\n        gamma,\n        delta,\n    );}\n"
    );
}

#[test]
fn range_formatter_can_target_switch_pattern_lists() {
    let source = "fn run(kind){switch kind {foo|bar=>1}}\n";
    let selection_start =
        u32::try_from(source.find("foo").expect("expected pattern start")).expect("offset");
    let selection_end =
        u32::try_from(source.find("=>").expect("expected arrow start")).expect("offset");
    let result = format_range(
        source,
        TextRange::new(selection_start.into(), selection_end.into()),
        &FormatOptions::default(),
    )
    .expect("expected range formatting result");

    assert_eq!(
        apply_range_edit(source, result.range, &result.text),
        "fn run(kind){switch kind {foo | bar=>1}}\n"
    );
}

#[test]
fn range_formatter_can_target_switch_arm_lists() {
    let source = "fn run(kind){switch kind {foo=>alpha+beta,bar=>gamma+delta}}\n";
    let selection_start =
        u32::try_from(source.find("foo").expect("expected switch arms")).expect("offset");
    let selection_end =
        u32::try_from(source.find("}").expect("expected switch end")).expect("offset");
    let result = format_range(
        source,
        TextRange::new(selection_start.into(), selection_end.into()),
        &FormatOptions::default(),
    )
    .expect("expected range formatting result");

    assert_eq!(
        apply_range_edit(source, result.range, &result.text),
        "fn run(kind){switch kind {foo => alpha + beta, bar => gamma + delta}}\n"
    );
}

#[test]
fn range_formatter_can_target_for_bindings() {
    let source = "fn run(){for (item,index) in values {index}}\n";
    let selection_start =
        u32::try_from(source.find("item").expect("expected bindings")).expect("offset");
    let selection_end =
        u32::try_from(source.find(") in").expect("expected binding end") + 1).expect("offset");
    let result = format_range(
        source,
        TextRange::new(selection_start.into(), selection_end.into()),
        &FormatOptions::default(),
    )
    .expect("expected range formatting result");

    assert_eq!(
        apply_range_edit(source, result.range, &result.text),
        "fn run(){for (item, index) in values {index}}\n"
    );
}

#[test]
fn range_formatter_can_target_do_conditions() {
    let source = "fn run(){do { work() } while ready&&steady}\n";
    let selection_start =
        u32::try_from(source.find("while").expect("expected do condition")).expect("offset");
    let selection_end = u32::try_from(source.len() - 2).expect("offset");
    let result = format_range(
        source,
        TextRange::new(selection_start.into(), selection_end.into()),
        &FormatOptions::default(),
    )
    .expect("expected range formatting result");

    assert_eq!(
        apply_range_edit(source, result.range, &result.text),
        "fn run(){do { work() } while ready && steady}\n"
    );
}

#[test]
fn range_formatter_can_target_catch_clauses() {
    let source = "fn run(){try { work() } catch (err){handle(err+1)}}\n";
    let selection_start =
        u32::try_from(source.find("catch").expect("expected catch clause")).expect("offset");
    let selection_end = u32::try_from(source.len() - 2).expect("offset");
    let result = format_range(
        source,
        TextRange::new(selection_start.into(), selection_end.into()),
        &FormatOptions::default(),
    )
    .expect("expected range formatting result");

    assert_eq!(
        apply_range_edit(source, result.range, &result.text),
        "fn run(){try { work() } catch (err) {\n        handle(err + 1)\n    }}\n"
    );
}

#[test]
fn range_formatter_can_target_alias_clauses() {
    let source = "import \"pkg\" as   helper;\n";
    let selection_start =
        u32::try_from(source.find("as").expect("expected alias clause")).expect("offset");
    let selection_end =
        u32::try_from(source.find(";").expect("expected alias end")).expect("offset");
    let result = format_range(
        source,
        TextRange::new(selection_start.into(), selection_end.into()),
        &FormatOptions::default(),
    )
    .expect("expected range formatting result");

    assert_eq!(
        apply_range_edit(source, result.range, &result.text),
        "import \"pkg\" as helper;\n"
    );
}

#[test]
fn range_formatter_can_target_else_branches() {
    let source = "fn run(){if ready { work() } else { fallback(1+2) }}\n";
    let selection_start =
        u32::try_from(source.find("else").expect("expected else branch")).expect("offset");
    let selection_end = u32::try_from(source.len() - 2).expect("offset");
    let result = format_range(
        source,
        TextRange::new(selection_start.into(), selection_end.into()),
        &FormatOptions::default(),
    )
    .expect("expected range formatting result");

    assert_eq!(
        apply_range_edit(source, result.range, &result.text),
        "fn run(){if ready { work() } else {\n        fallback(1 + 2)\n    }}\n"
    );
}
