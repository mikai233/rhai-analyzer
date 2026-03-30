use crate::{FormatOptions, format_text};
use rhai_syntax::parse_text;

mod config;
mod document;
mod layout;
mod range;
mod trivia;

fn assert_formats_to(source: &str, expected: &str) {
    let result = format_text(source, &FormatOptions::default());

    assert_eq!(result.text, expected);
    assert!(parse_text(&result.text).errors().is_empty());
}
