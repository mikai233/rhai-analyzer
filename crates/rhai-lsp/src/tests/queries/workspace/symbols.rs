use crate::Server;
use crate::tests::{assert_valid_rhai_syntax, file_url};

#[test]
fn workspace_symbol_queries_return_uri_backed_results() {
    let mut server = Server::new();
    let provider_uri = file_url("provider.rhai");
    let consumer_uri = file_url("consumer.rhai");
    let provider_text = "fn helper() {}\nconst VALUE = 1;";
    let consumer_text = "fn run() { helper(); }";

    assert_valid_rhai_syntax(provider_text);
    assert_valid_rhai_syntax(consumer_text);
    server
        .open_document(provider_uri.clone(), 1, provider_text)
        .expect("expected provider open to succeed");
    server
        .open_document(consumer_uri.clone(), 1, consumer_text)
        .expect("expected consumer open to succeed");

    let symbols = server
        .workspace_symbols("help")
        .expect("expected workspace symbols query to succeed");

    assert!(
        symbols
            .iter()
            .any(|symbol| symbol.uri == provider_uri && symbol.symbol.name == "helper"),
        "expected provider helper symbol, got {symbols:?}"
    );
}
