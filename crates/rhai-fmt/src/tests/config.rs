use crate::{ContainerLayoutStyle, FormatMode, FormatOptions, ImportSortOrder, IndentStyle};

#[test]
fn formatter_options_default_to_document_spaces_and_trailing_commas() {
    let options = FormatOptions::default();

    assert_eq!(options.mode, FormatMode::Document);
    assert_eq!(options.indent_style, IndentStyle::Spaces);
    assert_eq!(options.indent_width, 4);
    assert_eq!(options.max_line_length, 100);
    assert!(options.trailing_commas);
    assert!(options.final_newline);
    assert_eq!(options.container_layout, ContainerLayoutStyle::Auto);
    assert_eq!(options.import_sort_order, ImportSortOrder::Preserve);
}
