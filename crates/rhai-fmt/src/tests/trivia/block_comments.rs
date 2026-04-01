use crate::tests::assert_formats_to;

#[test]
fn formatter_preserves_multiline_block_comment_layout() {
    let source = "fn run(){\n/*\n * aligned block\n * comment\n */\nvalue\n}\n";

    let expected = "fn run() {\n    /*\n * aligned block\n * comment\n */\n    value\n}\n";

    assert_formats_to(source, expected);
}
#[test]
fn formatter_preserves_multiline_doc_block_comment_layout() {
    let source = "/**\n * documented API\n * keeps star alignment\n */\nfn run(){value}\n";

    let expected =
        "/**\n * documented API\n * keeps star alignment\n */\nfn run() {\n    value\n}\n";

    assert_formats_to(source, expected);
}
