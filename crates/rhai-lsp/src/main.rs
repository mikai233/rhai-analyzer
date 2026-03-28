fn main() {
    let server = rhai_lsp::Server::new();
    let capabilities = server.capabilities();

    eprintln!(
        "rhai-lsp skeleton ready (text sync: {:?})",
        capabilities.text_document_sync
    );
}
