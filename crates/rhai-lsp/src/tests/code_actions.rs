use crate::Server;
use crate::tests::{assert_valid_rhai_syntax, file_url, offset_in};

#[test]
fn auto_import_actions_are_not_exposed_for_workspace_exports() {
    let mut server = Server::new();
    let provider_uri = file_url("provider.rhai");
    let consumer_uri = file_url("consumer.rhai");
    let provider_text = "let helper = 1; export helper as shared_tools;";
    let consumer_text = "fn run() { shared_tools(); }";

    assert_valid_rhai_syntax(provider_text);
    assert_valid_rhai_syntax(consumer_text);

    server
        .open_document(provider_uri, 1, provider_text)
        .expect("expected provider open to succeed");
    server
        .open_document(consumer_uri.clone(), 1, consumer_text)
        .expect("expected consumer open to succeed");

    let actions = server
        .auto_import_actions(
            &consumer_uri,
            offset_in("fn run() { shared_tools(); }", "shared_tools"),
        )
        .expect("expected auto import actions");

    assert!(actions.is_empty());
}
