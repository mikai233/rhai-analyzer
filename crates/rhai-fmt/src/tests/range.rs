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

    assert_eq!(u32::from(result.range.start()), 9);
    assert_eq!(u32::from(result.range.end()), 28);
    assert_eq!(
        result.text,
        "\n    let value = 1 + 2;\n    value\n".to_owned()
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
        "fn run(){let values=[\n        alpha,\n        beta,\n        gamma,\n        delta,\n];}\n"
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
        "fn run(){helper(\n        alpha,\n        beta,\n        gamma,\n        delta,\n);}\n"
    );
}
