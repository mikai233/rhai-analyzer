use crate::{FormatOptions, format_range};
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
