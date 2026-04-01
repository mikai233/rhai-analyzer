use lsp_types::{DocumentChangeOperation, DocumentChanges, ResourceOp};

use crate::Server;
use crate::protocol::rename_to_workspace_edit;
use crate::tests::{assert_valid_rhai_syntax, file_url, offset_in};

#[test]
fn rename_updates_object_field_usages_across_files() {
    let mut server = Server::new();
    let provider_uri = file_url("provider.rhai");
    let consumer_uri = file_url("consumer.rhai");
    let provider_text = r#"
        export const DEFAULTS = #{
            name: "demo",
            watch: true,
        };
    "#;
    let consumer_text = r#"
        import "provider" as tools;
        let value = tools::DEFAULTS.name;
    "#;

    assert_valid_rhai_syntax(provider_text);
    assert_valid_rhai_syntax(consumer_text);
    server
        .open_document(provider_uri.clone(), 1, provider_text)
        .expect("expected provider open to succeed");
    server
        .open_document(consumer_uri.clone(), 1, consumer_text)
        .expect("expected consumer open to succeed");

    let prepared = server
        .rename(
            &provider_uri,
            offset_in(provider_text, "name: \"demo\""),
            "title".to_owned(),
        )
        .expect("expected rename query to succeed")
        .expect("expected prepared rename");
    assert!(
        prepared.plan.issues.is_empty(),
        "{:?}",
        prepared.plan.issues
    );

    let source_change = prepared
        .source_change
        .expect("expected object field rename source change");
    assert!(
        source_change.file_edits.len() >= 2,
        "expected provider+consumer edits, got {:?}",
        source_change.file_edits
    );
    assert!(
        source_change
            .file_edits
            .iter()
            .all(|file_edit| file_edit.edits.iter().all(|edit| edit.new_text == "title"))
    );
}

#[test]
fn rename_on_static_import_module_reference_returns_text_edits_and_file_rename() {
    let mut server = Server::new();
    let provider_uri = file_url("demo.rhai");
    let consumer_uri = file_url("consumer.rhai");
    let provider_text = "fn hello() {}\n";
    let consumer_text = "import \"demo\" as d;\n\nfn run() {\n    d::hello();\n}\n";

    assert_valid_rhai_syntax(provider_text);
    assert_valid_rhai_syntax(consumer_text);
    server
        .open_document(provider_uri.clone(), 1, provider_text)
        .expect("expected provider open to succeed");
    server
        .open_document(consumer_uri.clone(), 1, consumer_text)
        .expect("expected consumer open to succeed");

    let prepared = server
        .rename(
            &consumer_uri,
            offset_in(consumer_text, "\"demo\"") + 1,
            "renamed_demo".to_owned(),
        )
        .expect("expected rename query to succeed")
        .expect("expected prepared rename");
    let workspace_edit =
        rename_to_workspace_edit(&server, prepared).expect("expected workspace edit");
    let document_changes = workspace_edit
        .document_changes
        .expect("expected document changes");
    let DocumentChanges::Operations(document_changes) = document_changes else {
        panic!("expected operation-based workspace edit");
    };

    assert!(
        document_changes
            .iter()
            .any(|change| matches!(change, DocumentChangeOperation::Edit(_))),
        "expected text edits in workspace edit, got {document_changes:?}"
    );
    assert!(
        document_changes.iter().any(|change| matches!(
            change,
            DocumentChangeOperation::Op(ResourceOp::Rename(rename))
                if rename.new_uri.as_str().ends_with("/renamed_demo.rhai")
                    || rename.new_uri.as_str().ends_with("\\renamed_demo.rhai")
        )),
        "expected file rename in workspace edit, got {document_changes:?}"
    );
}

#[test]
fn static_import_module_can_be_renamed_twice_after_file_rename_notification() {
    let mut server = Server::new();
    let provider_uri = file_url("demo.rhai");
    let renamed_provider_uri = file_url("renamed_demo.rhai");
    let consumer_uri = file_url("consumer.rhai");
    let provider_text = "fn hello() {}\n";
    let consumer_text = "import \"demo\" as d;\n\nfn run() {\n    d::hello();\n}\n";
    let renamed_consumer_text = "import \"renamed_demo\" as d;\n\nfn run() {\n    d::hello();\n}\n";

    assert_valid_rhai_syntax(provider_text);
    assert_valid_rhai_syntax(consumer_text);
    assert_valid_rhai_syntax(renamed_consumer_text);
    server
        .open_document(provider_uri.clone(), 1, provider_text)
        .expect("expected provider open to succeed");
    server
        .open_document(consumer_uri.clone(), 1, consumer_text)
        .expect("expected consumer open to succeed");

    server
        .rename_workspace_file(&provider_uri, &renamed_provider_uri)
        .expect("expected file rename notification to succeed");
    server
        .change_document(consumer_uri.clone(), 2, renamed_consumer_text)
        .expect("expected consumer rename edit to succeed");

    let second = server
        .rename(
            &consumer_uri,
            offset_in(renamed_consumer_text, "\"renamed_demo\"") + 1,
            "demo_again".to_owned(),
        )
        .expect("expected second rename query to succeed");
    assert!(
        second.is_some(),
        "expected second static import rename to resolve"
    );
}
