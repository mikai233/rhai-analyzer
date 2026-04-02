use rhai_syntax::TextSize;

use crate::Server;
use crate::tests::{assert_valid_rhai_syntax, file_url, offset_in};

#[test]
fn prepare_rename_queries_flow_through_server() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        fn helper(value) {
            value
        }

        fn run() {
            helper(1);
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let prepared = server
        .prepare_rename(&uri, offset_in(text, "helper(1)") + 1)
        .expect("expected prepare rename query to succeed")
        .expect("expected prepared rename");
    let query_offset = TextSize::from(offset_in(text, "helper(1)") + 1);

    assert!(
        prepared
            .plan
            .targets
            .iter()
            .map(|target| target.focus_range)
            .chain(
                prepared
                    .plan
                    .occurrences
                    .iter()
                    .map(|occurrence| occurrence.range)
            )
            .any(|range| range.contains(query_offset))
    );
}

#[test]
fn prepare_rename_supports_static_import_module_paths() {
    let mut server = Server::new();
    let provider_uri = file_url("net/tcp.rhai");
    let consumer_uri = file_url("nested/consumer.rhai");
    let provider_text = "fn hello() {}\n";
    let consumer_text = r#"
        import "../net/tcp" as tcp;

        fn run() {
            tcp::hello();
        }
    "#;

    assert_valid_rhai_syntax(provider_text);
    assert_valid_rhai_syntax(consumer_text);
    server
        .open_document(provider_uri.clone(), 1, provider_text)
        .expect("expected provider open to succeed");
    server
        .open_document(consumer_uri.clone(), 1, consumer_text)
        .expect("expected consumer open to succeed");

    let offset = offset_in(consumer_text, "../net/tcp") + 4;
    let prepared = server
        .prepare_rename(&consumer_uri, offset)
        .expect("expected prepare rename query to succeed")
        .expect("expected prepared rename");
    let query_offset = TextSize::from(offset);

    assert!(
        prepared
            .plan
            .occurrences
            .iter()
            .map(|occurrence| occurrence.range)
            .any(|range| range.contains(query_offset)),
        "expected module string occurrence to cover the rename position: {prepared:?}"
    );
}
