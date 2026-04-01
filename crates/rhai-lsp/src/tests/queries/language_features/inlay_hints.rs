use lsp_types::{Position, Range};

use crate::tests::{assert_valid_rhai_syntax, file_url};
use crate::{InlayHintSettings, Server, ServerSettings};

#[test]
fn inlay_hints_queries_flow_through_server() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        fn helper() {
            1
        }

        fn run() {
            let result = helper();
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let hints = server
        .inlay_hints(&uri, None)
        .expect("expected inlay hints query to succeed");

    assert!(
        hints.iter().any(|hint| hint.label == " -> int"),
        "expected function return hint, got {hints:?}"
    );
    assert!(
        hints.iter().any(|hint| hint.label == ": int"),
        "expected inferred variable hint, got {hints:?}"
    );
}
#[test]
fn inlay_hints_queries_respect_server_settings() {
    let mut server = Server::new();
    server.configure_settings(ServerSettings {
        inlay_hints: InlayHintSettings {
            variables: false,
            parameters: true,
            return_types: false,
        },
        ..ServerSettings::default()
    });

    let uri = file_url("main.rhai");
    let text = r#"
        fn helper(value) {
            value
        }

        fn run() {
            let result = helper(1);
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let hints = server
        .inlay_hints(&uri, None)
        .expect("expected inlay hints query to succeed");

    assert!(
        hints.iter().any(|hint| hint.label == ": int"),
        "expected parameter hint, got {hints:?}"
    );
    assert!(
        !hints.iter().any(|hint| hint.label == " -> int"),
        "did not expect return-type hint, got {hints:?}"
    );
}
#[test]
fn inlay_hints_queries_can_be_scoped_to_a_requested_range() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        fn helper() {
            1
        }

        fn run() {
            let result = helper();
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let hints = server
        .inlay_hints(
            &uri,
            Some(Range {
                start: Position {
                    line: 5,
                    character: 0,
                },
                end: Position {
                    line: 7,
                    character: 0,
                },
            }),
        )
        .expect("expected inlay hints query to succeed");

    assert!(
        hints.iter().any(|hint| hint.label == ": int"),
        "expected variable hint, got {hints:?}"
    );
    assert!(
        !hints.iter().any(|hint| hint.label == " -> int"),
        "did not expect helper return hint outside range, got {hints:?}"
    );
}
