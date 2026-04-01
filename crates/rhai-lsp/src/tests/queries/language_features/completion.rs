use crate::Server;
use crate::tests::{assert_valid_rhai_syntax, file_url, offset_in};

#[test]
fn completion_queries_and_resolve_flow_through_server() {
    let mut server = Server::new();
    let provider_uri = file_url("provider.rhai");
    let consumer_uri = file_url("consumer.rhai");
    let provider_text = "/// shared helper docs\nfn shared_helper() {}";
    let consumer_text = r#"
        fn run() {
            shared
        }
    "#;

    assert_valid_rhai_syntax(provider_text);
    assert_valid_rhai_syntax(consumer_text);
    server
        .open_document(provider_uri, 1, provider_text)
        .expect("expected provider open to succeed");
    server
        .open_document(consumer_uri.clone(), 1, consumer_text)
        .expect("expected consumer open to succeed");

    let completion = server
        .completions(
            &consumer_uri,
            offset_in(consumer_text, "shared") + "shared".len() as u32,
        )
        .expect("expected completions query to succeed")
        .into_iter()
        .find(|item| item.label == "shared_helper")
        .expect("expected shared_helper completion");
    assert!(completion.docs.is_none());

    let resolved = server.resolve_completion(completion);
    assert_eq!(resolved.docs.as_deref(), Some("shared helper docs"));
}
#[test]
fn signature_help_queries_flow_through_server() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        /// @param left int
        /// @param right string
        /// @return bool
        fn check(left, right) {
            true
        }

        fn run() {
            check(1, value);
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let help = server
        .signature_help(&uri, offset_in(text, "value"))
        .expect("expected signature help query to succeed")
        .expect("expected signature help");

    assert_eq!(help.active_parameter, 1);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(
        help.signatures[0].label,
        "fn check(left: int, right: string) -> bool"
    );
}
