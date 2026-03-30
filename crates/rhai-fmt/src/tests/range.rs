use crate::{FormatOptions, format_range};
use rhai_syntax::{TextRange, TextSize};

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
