use crate::Server;
use crate::tests::{absolute_test_path, assert_valid_rhai_syntax, file_url};
use rhai_ide::{DiagnosticSeverity, DiagnosticTag};

#[test]
fn opening_document_returns_diagnostics_for_that_document() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");

    let updates = server
        .open_document(uri.clone(), 1, "let value = ;")
        .expect("expected open_document to succeed");

    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].uri, uri);
    assert_eq!(updates[0].version, Some(1));
    assert!(!updates[0].diagnostics.is_empty());
}

#[test]
fn changing_document_republishes_open_dependents_and_warms_hot_files() {
    let mut server = Server::new();
    let provider_uri = file_url("provider.rhai");
    let consumer_uri = file_url("consumer.rhai");
    let provider_text = "export const VALUE = 1;";
    let consumer_text = "import \"provider\" as tools;\ntools;\nfn run() {}";
    let renamed_provider_text = "export const VALUE = 2;";

    assert_valid_rhai_syntax(provider_text);
    assert_valid_rhai_syntax(consumer_text);
    assert_valid_rhai_syntax(renamed_provider_text);

    server
        .open_document(provider_uri.clone(), 1, provider_text)
        .expect("expected provider open to succeed");
    server
        .open_document(consumer_uri.clone(), 1, consumer_text)
        .expect("expected consumer open to succeed");

    let updates = server
        .change_document(provider_uri.clone(), 2, renamed_provider_text)
        .expect("expected provider change to succeed");

    assert!(updates.iter().any(|update| update.uri == provider_uri));
    let consumer_update = updates
        .iter()
        .find(|update| update.uri == consumer_uri)
        .expect("expected consumer diagnostics update");
    assert!(consumer_update.diagnostics.is_empty());

    let analysis = server.analysis_host().snapshot();
    let provider = analysis
        .file_id_for_path(&absolute_test_path("provider.rhai"))
        .expect("expected provider file id");
    let consumer = analysis
        .file_id_for_path(&absolute_test_path("consumer.rhai"))
        .expect("expected consumer file id");
    assert!(analysis.has_query_support(provider));
    assert!(analysis.has_query_support(consumer));
}

#[test]
fn closing_document_clears_diagnostics_and_unloads_file() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");

    server
        .open_document(uri.clone(), 1, "let value = ;")
        .expect("expected open_document to succeed");
    let updates = server.close_document(&uri);

    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].uri, uri);
    assert!(updates[0].diagnostics.is_empty());
    assert_eq!(updates[0].version, None);

    let analysis = server.analysis_host().snapshot();
    assert!(
        analysis
            .file_id_for_path(&absolute_test_path("main.rhai"))
            .is_none()
    );
}

#[test]
fn unused_diagnostics_are_published_as_warnings() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");

    let updates = server
        .open_document(uri, 1, "import \"shared_tools\" as tools;\nfn run() {}")
        .expect("expected open_document to succeed");

    let unused = updates
        .iter()
        .flat_map(|update| update.diagnostics.iter())
        .find(|diagnostic| diagnostic.message == "unused symbol `tools`")
        .expect("expected unused diagnostic");

    assert_eq!(unused.severity, DiagnosticSeverity::Warning);
    assert_eq!(unused.tags, vec![DiagnosticTag::Unnecessary]);
}
