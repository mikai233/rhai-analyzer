use crate::FormatOptions;
use crate::formatter::layout::doc::Doc;
use crate::formatter::layout::render::{render_doc, render_doc_with_indent};

#[test]
fn doc_builders_create_expected_nodes() {
    assert_eq!(Doc::nil(), Doc::Nil);
    assert_eq!(Doc::text("value"), Doc::Text("value".to_owned()));
    assert_eq!(Doc::text(" "), Doc::Text(" ".to_owned()));
    assert_eq!(Doc::hard_line(), Doc::HardLine);
    assert_eq!(Doc::soft_line(), Doc::SoftLine);
}

#[test]
fn renderer_concatenates_and_renders_breaks() {
    let rendered = render_doc(
        &Doc::concat(vec![
            Doc::text("fn run()"),
            Doc::hard_line(),
            Doc::text("{"),
            Doc::hard_line(),
            Doc::text("}"),
        ]),
        &FormatOptions::default(),
    );

    assert_eq!(rendered, "fn run()\n{\n}");
}

#[test]
fn renderer_breaks_groups_with_indentation() {
    let rendered = render_doc(
        &Doc::group(Doc::concat(vec![
            Doc::text("["),
            Doc::indent(
                1,
                Doc::concat(vec![
                    Doc::soft_line(),
                    Doc::concat(vec![
                        Doc::text("12345"),
                        Doc::text(","),
                        Doc::soft_line(),
                        Doc::text("67890"),
                        Doc::text(","),
                        Doc::soft_line(),
                        Doc::text("abcde"),
                    ]),
                    Doc::text(","),
                ]),
            ),
            Doc::soft_line(),
            Doc::text("]"),
        ])),
        &FormatOptions {
            max_line_length: 12,
            ..FormatOptions::default()
        },
    );

    assert_eq!(rendered, "[\n    12345,\n    67890,\n    abcde,\n]");
}

#[test]
fn renderer_applies_base_indent_to_fragments() {
    let rendered = render_doc_with_indent(
        &Doc::group(Doc::concat(vec![
            Doc::text("{"),
            Doc::indent(1, Doc::concat(vec![Doc::soft_line(), Doc::text("value")])),
            Doc::soft_line(),
            Doc::text("}"),
        ])),
        &FormatOptions {
            max_line_length: 4,
            ..FormatOptions::default()
        },
        1,
    );

    assert_eq!(rendered, "{\n        value\n    }");
}
