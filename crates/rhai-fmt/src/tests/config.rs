use crate::{FormatMode, FormatOptions, IndentStyle};

#[test]
fn formatter_options_default_to_document_spaces_and_trailing_commas() {
    let options = FormatOptions::default();

    assert_eq!(options.mode, FormatMode::Document);
    assert_eq!(options.indent_style, IndentStyle::Spaces);
    assert_eq!(options.indent_width, 4);
    assert_eq!(options.max_line_length, 100);
    assert!(options.trailing_commas);
}
