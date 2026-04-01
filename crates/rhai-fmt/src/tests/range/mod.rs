use crate::tests::apply_range_edit;
use crate::{ContainerLayoutStyle, FormatOptions, RangeFormatResult, format_range};
use rhai_syntax::{TextRange, TextSize};

mod basics;
mod clauses;
mod lists;

pub(crate) fn format_range_edit(
    source: &str,
    range: TextRange,
    options: &FormatOptions,
) -> RangeFormatResult {
    format_range(source, range, options).expect("expected range formatting result")
}

pub(crate) fn default_range(start: u32, end: u32) -> TextRange {
    TextRange::new(TextSize::from(start), TextSize::from(end))
}

pub(crate) fn assert_range_rewrites_to(
    source: &str,
    range: TextRange,
    options: &FormatOptions,
    expected: &str,
) {
    let result = format_range_edit(source, range, options);
    assert_eq!(
        apply_range_edit(source, result.range, &result.text),
        expected
    );
}

pub(crate) fn prefer_multiline_options() -> FormatOptions {
    FormatOptions {
        container_layout: ContainerLayoutStyle::PreferMultiLine,
        ..FormatOptions::default()
    }
}
