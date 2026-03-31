use crate::{FormatOptions, format_text};
use rhai_syntax::{TextRange, parse_text};

mod config;
mod corpus;
mod document;
mod guarantees;
mod layout;
mod range;
mod support;
mod trivia;

fn assert_formats_to(source: &str, expected: &str) {
    assert_formats_to_with_options(source, expected, &FormatOptions::default());
}

fn assert_formats_to_with_options(source: &str, expected: &str, options: &FormatOptions) {
    let result = format_text(source, options);

    assert_eq!(result.text, expected);
    assert!(parse_text(&result.text).errors().is_empty());
}

fn assert_parse_stable(text: &str) {
    let parse = parse_text(text);
    assert!(
        parse.errors().is_empty(),
        "expected parse-stable formatted output, got errors: {:?}\ntext:\n{}",
        parse.errors(),
        text
    );
}

fn assert_idempotent_with_options(source: &str, options: &FormatOptions) {
    let once = format_text(source, options);
    assert_parse_stable(&once.text);

    let twice = format_text(&once.text, options);
    assert_eq!(twice.text, once.text);
    assert!(
        !twice.changed,
        "expected second formatting pass to be stable\nfirst:\n{}\nsecond:\n{}",
        once.text, twice.text
    );
}

fn apply_range_edit(source: &str, range: TextRange, replacement: &str) -> String {
    let start = u32::from(range.start()) as usize;
    let end = u32::from(range.end()) as usize;
    format!("{}{}{}", &source[..start], replacement, &source[end..])
}
