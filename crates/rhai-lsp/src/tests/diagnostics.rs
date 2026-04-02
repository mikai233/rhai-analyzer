use crate::Server;
use crate::tests::{absolute_test_path, assert_valid_rhai_syntax, file_url};
use rhai_db::ProjectDiagnosticCode;
use rhai_hir::SemanticDiagnosticCode;
use rhai_ide::{DiagnosticSeverity, DiagnosticTag};
use rhai_syntax::SyntaxErrorCode;

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
    assert!(updates[0].diagnostics.iter().any(|diagnostic| {
        diagnostic.code == ProjectDiagnosticCode::Syntax(SyntaxErrorCode::ExpectedExpression)
    }));
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
        .find(|diagnostic| {
            diagnostic.code == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnusedSymbol)
        })
        .expect("expected unused diagnostic");

    assert_eq!(unused.severity, DiagnosticSeverity::Warning);
    assert_eq!(unused.tags, vec![DiagnosticTag::Unnecessary]);
}

#[test]
fn constant_condition_diagnostics_are_published_as_warnings() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");

    let updates = server
        .open_document(
            uri,
            1,
            r#"
                fn run() {
                    if true {
                        1;
                    } else {
                        2;
                    }

                    while false {
                        3;
                    }

                    do {
                        4;
                    } while false;

                    do {
                        5;
                    } until true;

                    while true {
                        break;
                    }

                    do {
                        break;
                    } while true;

                    do {
                        break;
                    } until false;

                    while true {
                        loop {
                            break;
                        }
                    }
                }
            "#,
        )
        .expect("expected open_document to succeed");

    let diagnostics = updates
        .iter()
        .flat_map(|update| update.diagnostics.iter())
        .filter(|diagnostic| {
            diagnostic.code
                == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::ConstantCondition)
        })
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 5, "{diagnostics:?}");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
    );
    assert!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message == "while condition is always true")
            .count()
            == 1
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.message != "do-while condition is always true")
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.message != "do-until condition is always false")
    );
}

#[test]
fn changing_importer_republishes_dependency_diagnostics_immediately() {
    let mut server = Server::new();
    let provider_uri = file_url("provider.rhai");
    let consumer_uri = file_url("consumer.rhai");
    let provider_text = r#"
        let defaults = #{ name: "demo" };
        fn make_config() {
            defaults
        }
    "#;
    let consumer_with_call = r#"
        import "provider" as tools;
        fn run() {
            tools::make_config();
        }
    "#;
    let consumer_without_call = r#"
        import "provider" as tools;
        fn run() {}
    "#;

    assert_valid_rhai_syntax(provider_text);
    assert_valid_rhai_syntax(consumer_with_call);
    assert_valid_rhai_syntax(consumer_without_call);

    server
        .open_document(provider_uri.clone(), 1, provider_text)
        .expect("expected provider open to succeed");
    let updates_with_call = server
        .open_document(consumer_uri.clone(), 1, consumer_with_call)
        .expect("expected consumer open to succeed");

    let provider_update_with_call = updates_with_call
        .iter()
        .find(|update| update.uri == provider_uri)
        .expect("expected provider diagnostics update when importer introduces regular call");
    assert!(
        provider_update_with_call
            .diagnostics
            .iter()
            .any(|diagnostic| {
                diagnostic.code
                    == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedName)
            })
    );

    let updates_without_call = server
        .change_document(consumer_uri, 2, consumer_without_call)
        .expect("expected consumer change to succeed");
    let provider_update_without_call = updates_without_call
        .iter()
        .find(|update| update.uri == provider_uri)
        .expect("expected provider diagnostics update when importer removes regular call");
    assert!(
        !provider_update_without_call
            .diagnostics
            .iter()
            .any(|diagnostic| {
                diagnostic.code
                    == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedName)
            }),
        "expected unresolved capture diagnostic to clear without regular calls, got {:?}",
        provider_update_without_call.diagnostics
    );
}
