use lsp_types::{CodeActionKind, Position, Range};

use crate::Server;
use crate::protocol::diagnostic_to_lsp;
use crate::tests::{assert_valid_rhai_syntax, file_url};

#[test]
fn code_actions_include_source_actions_and_resolve_to_workspace_edits() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        import "z" as z;
        import "a" as a;

        fn run() {
            a::work();
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let actions = server
        .code_actions(
            &uri,
            Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 5,
                    character: 0,
                },
            },
            &[],
            Some(&[CodeActionKind::SOURCE_ORGANIZE_IMPORTS]),
        )
        .expect("expected code actions query to succeed");

    let organize = actions
        .into_iter()
        .find(|action| action.kind == CodeActionKind::SOURCE_ORGANIZE_IMPORTS)
        .expect("expected organize imports action");
    assert!(
        !organize.is_preferred,
        "expected source organize imports action to avoid preferred quick-fix semantics"
    );

    let payload = crate::protocol::CodeActionResolvePayload {
        uri: uri.to_string(),
        request_range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 5,
                character: 0,
            },
        },
        id: organize.id,
        kind: organize.kind.as_str().to_owned(),
        title: organize.title,
        target_start: u32::from(organize.target.start()),
        target_end: u32::from(organize.target.end()),
    };

    let resolved = server
        .resolve_code_action(&payload)
        .expect("expected code action resolve to succeed")
        .expect("expected resolved action");
    assert!(
        !resolved.source_change.file_edits.is_empty(),
        "expected resolved code action to contain file edits"
    );
}
#[test]
fn code_actions_prefer_diagnostic_quickfixes_and_attach_matching_diagnostics() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = "import \"shared_tools\" as tools;\nfn run() {}\n";

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let snapshot = server.analysis_host().snapshot();
    let file_id = snapshot
        .file_id_for_path(&std::env::current_dir().expect("cwd").join("main.rhai"))
        .expect("expected main.rhai file id");
    let diagnostics = snapshot
        .diagnostics(file_id)
        .iter()
        .filter_map(|diagnostic| diagnostic_to_lsp(text, diagnostic))
        .collect::<Vec<_>>();

    let actions = server
        .code_actions(
            &uri,
            Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 1,
                    character: 0,
                },
            },
            &diagnostics,
            Some(&[CodeActionKind::QUICKFIX]),
        )
        .expect("expected code actions query to succeed");

    let remove_unused = actions
        .into_iter()
        .find(|action| action.id == "import.remove_unused")
        .expect("expected remove unused import quickfix");
    assert!(remove_unused.is_preferred);
    assert_eq!(remove_unused.diagnostics.len(), 1);
    assert_eq!(
        remove_unused.diagnostics[0].message,
        "unused symbol `tools`"
    );
}
