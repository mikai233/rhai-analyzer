use lsp_types::{FormattingOptions, Position, Range};
use rhai_fmt::{ContainerLayoutStyle, ImportSortOrder};

use crate::tests::queries::default_formatting_options;
use crate::tests::{assert_valid_rhai_syntax, file_url};
use crate::{Server, ServerSettings};

#[test]
fn on_type_formatting_queries_reformat_current_structure_for_statement_terminators() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = "fn run(){\nlet value=1;\n}\n";

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let edits = server
        .format_on_type(
            &uri,
            Position {
                line: 2,
                character: 1,
            },
            "}",
            default_formatting_options(),
        )
        .expect("expected on-type formatting query to succeed")
        .expect("expected on-type formatting edits");

    assert!(
        !edits.is_empty(),
        "expected non-empty on-type formatting edits"
    );
}
#[test]
fn document_formatting_queries_flow_through_server() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = "fn run(){let value=1+2;value}\n";

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let edits = server
        .format_document(&uri, default_formatting_options())
        .expect("expected format document query to succeed")
        .expect("expected formatting edits");
    let [edit] = edits.as_slice() else {
        panic!("expected a single formatting edit");
    };

    assert_eq!(edit.range.start.line, 0);
    assert_eq!(edit.range.start.character, 0);
    assert_eq!(edit.range.end.line, 1);
    assert_eq!(edit.range.end.character, 0);
    assert_eq!(
        edit.new_text,
        "fn run() {\n    let value = 1 + 2;\n    value\n}\n"
    );
}
#[test]
fn document_range_formatting_queries_flow_through_server() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = "let prefix = 1;\nfn run(){let value=1+2;value}\n";

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let edits = server
        .format_range(
            &uri,
            Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 2,
                    character: 0,
                },
            },
            default_formatting_options(),
        )
        .expect("expected format range query to succeed")
        .expect("expected range formatting edits");
    let [edit] = edits.as_slice() else {
        panic!("expected a single formatting edit");
    };

    assert_eq!(edit.range.start.line, 1);
    assert_eq!(edit.range.start.character, 0);
    assert!(edit.range.end.line <= 2);
    assert!(edit.new_text.contains("fn run"));
    assert!(edit.new_text.contains("let value = 1 + 2;"));
}
#[test]
fn document_formatting_queries_apply_request_and_server_formatting_options() {
    let mut server = Server::new();
    server.configure_settings(ServerSettings {
        formatter: crate::FormatterSettings {
            max_line_length: 12,
            trailing_commas: false,
            final_newline: false,
            container_layout: ContainerLayoutStyle::Auto,
            import_sort_order: ImportSortOrder::Preserve,
        },
        ..ServerSettings::default()
    });

    let uri = file_url("main.rhai");
    let text = "fn run(){let values=[12345,67890,abcde];}\n";

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let edits = server
        .format_document(
            &uri,
            FormattingOptions {
                tab_size: 2,
                insert_spaces: false,
                ..default_formatting_options()
            },
        )
        .expect("expected format document query to succeed")
        .expect("expected formatting edits");
    let [edit] = edits.as_slice() else {
        panic!("expected a single formatting edit");
    };

    assert!(edit.new_text.contains("\tlet values = ["));
    assert!(edit.new_text.contains("\t\t12345,"));
    assert!(edit.new_text.contains("\t\tabcde\n\t];"));
    assert!(!edit.new_text.contains("\t\tabcde,\n\t];"));
    assert!(!edit.new_text.ends_with('\n'));
}
#[test]
fn document_formatting_queries_apply_container_layout_preferences() {
    let mut server = Server::new();
    server.configure_settings(ServerSettings {
        formatter: crate::FormatterSettings {
            container_layout: ContainerLayoutStyle::PreferMultiLine,
            ..crate::FormatterSettings::default()
        },
        ..ServerSettings::default()
    });

    let uri = file_url("main.rhai");
    let text = "fn run(){let values=[1,2,3]; helper(alpha,beta);}\n";

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let edits = server
        .format_document(&uri, default_formatting_options())
        .expect("expected format document query to succeed")
        .expect("expected formatting edits");
    let [edit] = edits.as_slice() else {
        panic!("expected a single formatting edit");
    };

    assert!(edit.new_text.contains("let values = [\n"));
    assert!(edit.new_text.contains("helper(\n"));
}
#[test]
fn document_formatting_queries_apply_import_sorting_preferences() {
    let mut server = Server::new();
    server.configure_settings(ServerSettings {
        formatter: crate::FormatterSettings {
            import_sort_order: ImportSortOrder::ModulePath,
            ..crate::FormatterSettings::default()
        },
        ..ServerSettings::default()
    });

    let uri = file_url("main.rhai");
    let text =
        "import \"zebra\" as zebra;\nimport \"alpha\";\nimport \"beta\" as beta;\nfn run(){}\n";

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let edits = server
        .format_document(&uri, default_formatting_options())
        .expect("expected format document query to succeed")
        .expect("expected formatting edits");
    let [edit] = edits.as_slice() else {
        panic!("expected a single formatting edit");
    };

    assert!(
        edit.new_text.starts_with(
            "import \"alpha\";\nimport \"beta\" as beta;\nimport \"zebra\" as zebra;\n"
        )
    );
}
